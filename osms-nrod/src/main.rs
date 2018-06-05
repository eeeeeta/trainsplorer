extern crate stomp;
extern crate serde_json;
extern crate osms_db;
extern crate ntrod_types;
extern crate postgres;
#[macro_use] extern crate log;
extern crate toml;
extern crate fern;
#[macro_use] extern crate serde_derive;
extern crate futures;
extern crate tokio_core;
#[macro_use] extern crate failure;
extern crate cadence;
extern crate chrono;

use stomp::session::{SessionEvent, Session};
use stomp::header::Header;
use stomp::session_builder::SessionBuilder;
use stomp::subscription::AckOrNack;
use stomp::connection::*;
use std::net::UdpSocket;
use std::env;
use std::fs::File;
use std::io::Read;
use std::collections::HashMap;
use tokio_core::reactor::{Timeout, Handle, Core};
use std::time::Duration;
use futures::*;
use cadence::prelude::*;
use cadence::{StatsdClient, QueuingMetricSink, BufferedUdpMetricSink, DEFAULT_PORT};

use ntrod_types::movements::{Records, MvtBody};
use postgres::{Connection, TlsMode};
use std::borrow::Cow;

mod live;

#[derive(Deserialize)]
pub struct Config {
    database_url: String,
    #[serde(default)]
    statsd_url: Option<String>,
    username: String,
    password: String,
    #[serde(default)]
    log_level_general: Option<String>,
    #[serde(default)]
    log_level: HashMap<String, String>,
    #[serde(default)]
    nrod_url: Option<String>,
    #[serde(default)]
    nrod_port: Option<u16>
}
pub struct NtrodProcessor {
    sess: Session,
    conn: Connection,
    hdl: Handle,
    timeout: Option<Timeout>,
    timeout_ms: u64,
    metrics: Option<StatsdClient>
}
impl NtrodProcessor {
    fn incr(&mut self, dest: &str) {
        if let Some(ref mut metrics) = self.metrics {
            if let Err(e) = metrics.incr(dest) {
                error!("failed to update metrics: {:?}", e);
            }
        }
    }
    fn on_ntrod_data(&mut self, st: Cow<str>) {
        use self::MvtBody::*;

        self.incr("message_batch.recv");
        let recs: Result<Records, _> = serde_json::from_str(&st);
        match recs {
            Ok(r) => {
                self.incr("message_batch.parsed");
                for record in r {
                    self.incr("messages.recv");
                    let dest = match record.body {
                        Activation(_) => "messages_activation",
                        Cancellation(_) => "messages_cancellation",
                        Movement(_) => "messages_movement",
                        Reinstatement(_) => "messages_reinstatement",
                        ChangeOfOrigin(_) => "messages_change_of_origin",
                        ChangeOfIdentity(_) => "messages_change_of_identity"
                    };
                    self.incr(&format!("{}.recv", dest));
                    match live::process_ntrod_event(&self.conn, record) {
                        Err(e) => {
                            self.incr("messages.fail");
                            self.incr(&format!("{}.fail", dest));
                            error!("Error processing: {}", e);
                        },
                        _ => {
                            self.incr("messages.processed");
                            self.incr(&format!("{}.processed", dest));
                        }
                    }
                }
            },
            Err(e) => {
                self.incr("message_batch.parse_errors");
                error!("### PARSE ERROR ###\nerr: {}\ndata: {}", e, &st);
            }
        }
    }
}
impl Future for NtrodProcessor {
    type Item = ();
    type Error = ::failure::Error;

    fn poll(&mut self) -> Poll<(), Self::Error> {
        use self::SessionEvent::*;

        let tm = self.timeout
            .as_mut()
            .map(|t| t.poll())
            .unwrap_or(Ok(Async::NotReady))?;

        if let Async::Ready(_) = tm {
            info!("Reconnecting...");
            self.sess.reconnect().unwrap();
            self.timeout = None;
        }

        if let Async::Ready(ev) = self.sess.poll()? {
            let ev = ev.unwrap();
            match ev {
                Connected => {
                    info!("Connected to NTROD!");
                    self.timeout = None;
                    self.timeout_ms = 1000;
                    self.sess.subscription("/topic/TRAIN_MVT_ALL_TOC")
                        .with(Header::new("activemq.subscriptionName", "osms-nrod"))
                        .start();
                },
                ErrorFrame(fr) => {
                    error!("Error frame, reconnecting: {:?}", fr);
                    self.sess.reconnect().unwrap();
                },
                Message { destination, frame, .. } => {
                    if destination == "/topic/TRAIN_MVT_ALL_TOC" {
                        let st = String::from_utf8_lossy(&frame.body);
                        self.on_ntrod_data(st);
                    }
                    self.sess.acknowledge_frame(&frame, AckOrNack::Ack);
                },
                Disconnected(reason) => {
                    error!("disconnected: {:?}", reason);
                    error!("reconnecting in {} ms", self.timeout_ms);
                    let mut tm = Timeout::new(Duration::from_millis(self.timeout_ms), &self.hdl)?;
                    tm.poll()?;
                    self.timeout = Some(tm);
                    self.timeout_ms *= 2;
                },
                _ => {}
            }
        }
        Ok(Async::NotReady)
    }
}
/*
fn on_message(hdl: &mut LoginHandler, _: &mut Session<LoginHandler>, fr: &Frame) -> AckOrNack {
    let st = String::from_utf8_lossy(&fr.body);
    let recs: Result<Records, _> = serde_json::from_str(&st);
    match recs {
        Ok(r) => {
            for record in r {
                match live::process_ntrod_event(&hdl.0, record) {
                    Err(e) => {
                        error!("Error processing: {}", e);
                    },
                    _ => {
                    }
                }
            }
        },
        Err(e) => {
            error!("### PARSE ERROR ###\nerr: {}\ndata: {}", e, &st);
        }
    }
    AckOrNack::Ack
}
struct LoginHandler(Connection);
impl Handler for LoginHandler {
    fn on_connected(&mut self, sess: &mut Session<Self>, _: &Frame) {
        info!("Connection established.");
        sess.subscription("/topic/TRAIN_MVT_ALL_TOC", on_message).start().unwrap();
    }
    fn on_error(&mut self, _: &mut Session<Self>, frame: &Frame) {
        error!("Whoops: {}", frame);
    }
    fn on_disconnected(&mut self, _: &mut Session<Self>) {
        warn!("Disconnected.")
    }
}
*/
fn main() {
    println!("osms-nrod starting");
    let args = env::args().skip(1).collect::<Vec<_>>();
    let path = args.get(0).map(|x| x as &str).unwrap_or("config.toml");
    println!("Loading config from file {}...", path);
    let mut file = File::open(path).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    println!("Parsing config...");
    let conf: Config = toml::de::from_str(&contents).unwrap();
    let log_level_g: log::LogLevelFilter = conf.log_level_general
        .as_ref()
        .map(|x| x as &str)
        .unwrap_or("INFO")
        .parse()
        .unwrap();
    println!("General log level: {}", log_level_g);
    let mut logger = fern::Dispatch::new()
        .format(|out, msg, record| {
            out.finish(format_args!("[{} {}] {}",
                                    record.target(),
                                    record.level(),
                                    msg))
        })
        .level(log_level_g);
    for (k, v) in conf.log_level.iter() {
        println!("Log level for {} is {}", k, v);
        logger = logger.level_for(k.clone(), v.parse().unwrap());
    }
    logger
        .chain(std::io::stdout())
        .apply()
        .unwrap();
    let mut metrics = None;
    if let Some(ref url) = conf.statsd_url {
        info!("Initialising metrics...");
        let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
        socket.set_nonblocking(true).unwrap();

        let host = (url as &str, DEFAULT_PORT);
        let udp_sink = BufferedUdpMetricSink::from(host, socket).unwrap();
        let queuing_sink = QueuingMetricSink::from(udp_sink);
        metrics = Some(StatsdClient::from_sink("ntrod", queuing_sink));
    }
    info!("Connecting to database...");
    let conn = Connection::connect(conf.database_url, TlsMode::None).unwrap();
    conn.execute("SET application_name TO 'osms-nrod';", &[]).unwrap();
    info!("Running client...");
    let mut core = Core::new().unwrap();
    let hdl = core.handle();
    let nrod_url = conf.nrod_url.as_ref().map(|x| x as &str).unwrap_or("datafeeds.networkrail.co.uk");
    let sess = SessionBuilder::new(nrod_url, conf.nrod_port.unwrap_or(61618))
        .with(Credentials(&conf.username, &conf.password))
        .with(Header::new("client-id", "eta@theta.eu.org"))
        .with(HeartBeat(5_000, 2_000))
        .start(hdl.clone())
        .unwrap();
    let p = NtrodProcessor { conn, sess, hdl, timeout: None, timeout_ms: 1000, metrics };
    core.run(p).unwrap();
}
