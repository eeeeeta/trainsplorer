extern crate stomp;
extern crate serde_json;
#[macro_use] extern crate osms_db;
extern crate ntrod_types;
#[macro_use] extern crate log;
extern crate toml;
extern crate fern;
#[macro_use] extern crate serde_derive;
extern crate futures;
extern crate tokio_core;
#[macro_use] extern crate failure;
extern crate cadence;
extern crate chrono;
extern crate r2d2;
extern crate r2d2_postgres;
extern crate crossbeam_channel;

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
use std::sync::Arc;
use crossbeam_channel::{Sender, Receiver};

use ntrod_types::movements::{Records, MvtBody};
use ntrod_types::vstp::Record as VstpRecord;
use r2d2_postgres::{PostgresConnectionManager, TlsMode};

mod errors;
mod live;

#[derive(Deserialize)]
pub struct Config {
    database_url: String,
    #[serde(default)]
    statsd_url: Option<String>,
    username: String,
    password: String,
    n_threads: u32,
    #[serde(default)]
    log_level_general: Option<String>,
    #[serde(default)]
    log_level: HashMap<String, String>,
    #[serde(default)]
    nrod_url: Option<String>,
    #[serde(default)]
    nrod_port: Option<u16>
}
pub enum WorkerMessage {
    Movement(String),
    Vstp(String)
}
pub struct NtrodProcessor {
    sess: Session,
    hdl: Handle,
    timeout: Option<Timeout>,
    timeout_ms: u64,
    tx: Sender<WorkerMessage>
}
pub struct NtrodWorker {
    metrics: Option<Arc<StatsdClient>>,
    pool: r2d2::Pool<PostgresConnectionManager>,
    rx: Receiver<WorkerMessage> 
}
impl NtrodWorker {
    fn incr(&mut self, dest: &str) {
        if let Some(ref mut m) = self.metrics {
            if let Err(e) = m.incr(dest) {
                error!("failed to update metrics: {:?}", e);
            }
        }
    }
    fn run(&mut self) {
        loop {
            let data = self.rx.recv().unwrap();
            match data {
                WorkerMessage::Movement(data) => self.on_movement(data),
                WorkerMessage::Vstp(data) => self.on_vstp(data)
            }
        }
    }
    fn on_vstp(&mut self, st: String) {
        self.incr("vstp.recv");
        let conn = self.pool.get().unwrap();
        let msg: Result<VstpRecord, _> = serde_json::from_str(&st);
        match msg {
            Ok(record) => {
                self.incr("vstp.parsed");
                match live::process_vstp(&*conn, record) {
                    Err(e) => {
                        self.incr("vstp.fail");
                        error!("Error processing: {}", e);
                    },
                    _ => {
                        self.incr("vstp.processed");
                    }
                }
            },
            Err(e) => {
                self.incr("vstp.parse_errors");
                error!("### VSTP PARSE ERROR ###\nerr: {}\ndata: {}", e, &st);
            }
        }
    }
    fn on_movement(&mut self, st: String) {
        use self::MvtBody::*;

        self.incr("message_batch.recv");
        let conn = self.pool.get().unwrap();
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
                        ChangeOfIdentity(_) => "messages_change_of_identity",
                        ChangeOfLocation(_) => "messages_change_of_location",
                        Unknown(_) => "messages_unknown"
                    };
                    self.incr(&format!("{}.recv", dest));
                    match live::process_ntrod_event(&*conn, record) {
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
                    self.sess.subscription("/topic/VSTP_ALL")
                        .with(Header::new("activemq.subscriptionName", "osms-nrod-vstp"))
                        .start();
                },
                ErrorFrame(fr) => {
                    error!("Error frame, reconnecting: {:?}", fr);
                    self.sess.reconnect().unwrap();
                },
                Message { destination, frame, .. } => {
                    if destination == "/topic/TRAIN_MVT_ALL_TOC" {
                        let st = String::from_utf8_lossy(&frame.body);
                        self.tx.send(WorkerMessage::Movement(st.into())).unwrap();
                    }
                    if destination == "/topic/VSTP_ALL" {
                        let st = String::from_utf8_lossy(&frame.body);
                        self.tx.send(WorkerMessage::Vstp(st.into())).unwrap();
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
type Conn = <PostgresConnectionManager as r2d2::ManageConnection>::Connection;

#[derive(Debug)]
pub struct AppNameSetter;
impl<E> r2d2::CustomizeConnection<Conn, E> for AppNameSetter {
    fn on_acquire(&self, conn: &mut Conn) -> Result<(), E> {
        // FIXME: this unwrap isn't that great, it's a copypasta from osms-web
        conn.execute("SET application_name TO 'osms-nrod';", &[]).unwrap();
        Ok(())
    }
}
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
        metrics = Some(Arc::new(StatsdClient::from_sink("ntrod", queuing_sink)));
    }
    info!("Connecting to database...");
    let manager = PostgresConnectionManager::new(conf.database_url, TlsMode::None).unwrap();
    let pool = r2d2::Pool::builder()
        .connection_customizer(Box::new(AppNameSetter))
        .build(manager)
        .unwrap();
    info!("Initialising session...");
    let mut core = Core::new().unwrap();
    let hdl = core.handle();
    let nrod_url = conf.nrod_url.as_ref().map(|x| x as &str).unwrap_or("datafeeds.networkrail.co.uk");
    let sess = SessionBuilder::new(nrod_url, conf.nrod_port.unwrap_or(61618))
        .with(Credentials(&conf.username, &conf.password))
        .with(Header::new("client-id", "eta@theta.eu.org"))
        .with(HeartBeat(5_000, 2_000))
        .start(hdl.clone())
        .unwrap();
    let (tx, rx) = crossbeam_channel::unbounded();
    info!("Spawning {} worker threads...", conf.n_threads);
    for n in 0..conf.n_threads {
        let mut worker = NtrodWorker {
            pool: pool.clone(),
            metrics: metrics.clone(),
            rx: rx.clone()
        };
        ::std::thread::spawn(move || {
            info!("Worker thread {} running.", n);
            worker.run();
        });
    }
    info!("Running client...");
    let p = NtrodProcessor { tx, sess, hdl, timeout: None, timeout_ms: 1000 };
    core.run(p).unwrap();
}
