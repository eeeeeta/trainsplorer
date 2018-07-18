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
extern crate darwin_types;
extern crate flate2;

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
use flate2::read::GzDecoder;

use ntrod_types::movements::{Records, MvtBody};
use ntrod_types::vstp::Record as VstpRecord;
use r2d2_postgres::{PostgresConnectionManager, TlsMode};

mod errors;
mod live;
mod darwin;

#[derive(Deserialize)]
pub struct Config {
    database_url: String,
    #[serde(default)]
    statsd_url: Option<String>,
    username: String,
    password: String,
    darwin_username: String,
    darwin_password: String,
    darwin_queue_name: String,
    n_threads: u32,
    #[serde(default)]
    log_level_general: Option<String>,
    #[serde(default)]
    log_level: HashMap<String, String>,
    #[serde(default)]
    nrod_url: Option<String>,
    #[serde(default)]
    nrod_port: Option<u16>,
    #[serde(default)]
    darwin_url: Option<String>,
    #[serde(default)]
    darwin_port: Option<u16>
}
pub enum WorkerMessage {
    Movement(String),
    Vstp(String),
    Darwin(Vec<u8>)
}
pub struct NtrodProcessor {
    sess: Session,
    hdl: Handle,
    timeout: Option<Timeout>,
    timeout_ms: u64,
    tx: Sender<WorkerMessage>,
    darwin_qn: Option<String>
}
pub struct NtrodWorker {
    metrics: Option<Arc<StatsdClient>>,
    pub pool: r2d2::Pool<PostgresConnectionManager>,
    rx: Receiver<WorkerMessage> 
}
impl NtrodWorker {
    pub fn incr(&mut self, dest: &str) {
        if let Some(ref mut m) = self.metrics {
            if let Err(e) = m.incr(dest) {
                error!("failed to update metrics: {:?}", e);
            }
        }
    }
    pub fn latency(&mut self, dest: &str, dur: Duration) {
        if let Some(ref mut m) = self.metrics {
            if let Err(e) = m.time_duration(dest, dur) {
                error!("failed to update latency metrics: {:?}", e);
            }
        }
    }
    fn run(&mut self) {
        loop {
            let data = self.rx.recv().unwrap();
            match data {
                WorkerMessage::Movement(data) => self.on_movement(data),
                WorkerMessage::Vstp(data) => self.on_vstp(data),
                WorkerMessage::Darwin(data) => self.on_darwin(data)
            }
        }
    }
    fn on_darwin(&mut self, data: Vec<u8>) {
        use std::io::Read;

        self.incr("darwin.recv");
        let mut gz = GzDecoder::new(&data as &[u8]);
        let mut s = String::new();
        match gz.read_to_string(&mut s) {
            Ok(_) => {
                trace!("Got frame: {}", s);
                match darwin_types::parse_pport_document(s.as_bytes()) {
                    Ok(doc) => {
                        self.incr("darwin.parsed");
                        match darwin::process_darwin_pport(self, doc) {
                            Ok(_) => self.incr("darwin.processed"),
                            Err(e) => {
                                e.send_to_stats("darwin.fails", self);
                                error!("Failed to process pport: {}", e);
                                self.incr("darwin.fail");
                            }
                        }
                    },
                    Err(e) => {
                        error!("Failed to parse Darwin doc: {}", e);
                        self.incr("darwin.parse_errors");
                    }
                }
            },
            Err(e) => {
                self.incr("darwin.deflate_errors");
                error!("Failed to deflate frame: {}", e);
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
                        e.send_to_stats("vstp.fails", self);
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
                    match live::process_ntrod_event(&*conn, self, record) {
                        Err(e) => {
                            self.incr("messages.fail");
                            self.incr(&format!("{}.fail", dest));
                            e.send_to_stats("nrod.fails", self);
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
impl NtrodProcessor {
    fn service_name(&self) -> &'static str {
        if self.darwin_qn.is_some() {
            "Darwin"
        }
        else {
            "NTROD"
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
            info!("Reconnecting to {}...", self.service_name());
            self.sess.reconnect().unwrap();
            self.timeout = None;
        }

        if let Async::Ready(ev) = self.sess.poll()? {
            let ev = ev.unwrap();
            match ev {
                Connected => {
                    self.timeout = None;
                    self.timeout_ms = 1000;
                    if let Some(ref qn) = self.darwin_qn {
                        info!("Connected to Darwin; subscribing...");
                        self.sess.subscription(&format!("/queue/{}", qn))
                            .start();
                    }
                    else {
                        info!("Connected to NTROD; subscribing...");
                        self.sess.subscription("/topic/TRAIN_MVT_ALL_TOC")
                            .with(Header::new("activemq.subscriptionName", "osms-nrod"))
                            .start();
                        self.sess.subscription("/topic/VSTP_ALL")
                            .with(Header::new("activemq.subscriptionName", "osms-nrod-vstp"))
                            .start();
                    }
                },
                ErrorFrame(fr) => {
                    error!("Error frame from {}, reconnecting: {:?}", self.service_name(), fr);
                    self.sess.reconnect().unwrap();
                },
                Message { destination, frame, .. } => {
                    if self.darwin_qn.is_none() {
                        debug!("Got a NTROD message addressed to {}", destination);
                        if destination == "/topic/TRAIN_MVT_ALL_TOC" {
                            let st = String::from_utf8_lossy(&frame.body);
                            self.tx.send(WorkerMessage::Movement(st.into()));
                        }
                        if destination == "/topic/VSTP_ALL" {
                            let st = String::from_utf8_lossy(&frame.body);
                            self.tx.send(WorkerMessage::Vstp(st.into()));
                        }
                        self.sess.acknowledge_frame(&frame, AckOrNack::Ack);
                    }
                    else {
                        self.sess.acknowledge_frame(&frame, AckOrNack::Ack);
                        debug!("Got a Darwin message");
                        self.tx.send(WorkerMessage::Darwin(frame.body));
                    }
                },
                Disconnected(reason) => {
                    error!("disconnected from {}: {:?}", self.service_name(), reason);
                    error!("reconnecting to {} in {} ms", self.service_name(), self.timeout_ms);
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
    let log_level_g: log::LevelFilter = conf.log_level_general
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
    info!("Initialising database...");
    osms_db::db::initialize_database(&*pool.get().unwrap())
        .unwrap();
    info!("Initialising NTROD session...");
    let mut core = Core::new().unwrap();
    let hdl = core.handle();
    let nrod_url = conf.nrod_url.as_ref().map(|x| x as &str).unwrap_or("datafeeds.networkrail.co.uk");
    let nrod_sess = SessionBuilder::new(nrod_url, conf.nrod_port.unwrap_or(61618))
        .with(Credentials(&conf.username, &conf.password))
        .with(Header::new("client-id", "eta@theta.eu.org"))
        .with(HeartBeat(5_000, 2_000))
        .start(hdl.clone())
        .unwrap();
    info!("Initialising Darwin session...");
    let darwin_url = conf.darwin_url.as_ref().map(|x| x as &str).unwrap_or("datafeeds.nationalrail.co.uk");
    let darwin_sess = SessionBuilder::new(darwin_url, conf.darwin_port.unwrap_or(61613))
        .with(Credentials(&conf.darwin_username, &conf.darwin_password))
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
    info!("Running clients...");
    let nrod = NtrodProcessor { tx: tx.clone(), sess: nrod_sess, hdl: hdl.clone(), timeout: None, timeout_ms: 1000, darwin_qn: None };
    let darwin = NtrodProcessor { tx, sess: darwin_sess, hdl, timeout: None, timeout_ms: 1000, darwin_qn: Some(conf.darwin_queue_name) };
    core.run(nrod.join(darwin)).unwrap();
}
