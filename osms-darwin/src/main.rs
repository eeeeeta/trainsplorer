extern crate stomp;
#[macro_use] extern crate osms_db;
#[macro_use] extern crate log;
extern crate envy;
extern crate fern;
#[macro_use] extern crate serde_derive;
extern crate futures;
extern crate tokio_core;
#[macro_use] extern crate failure;
extern crate flate2;
extern crate darwin_types;

mod conf;
use conf::DarwinConfig;

use stomp::session::{SessionEvent, Session};
use stomp::header::Header;
use stomp::session_builder::SessionBuilder;
use stomp::subscription::AckOrNack;
use stomp::connection::*;
use tokio_core::reactor::{Timeout, Handle, Core};
use std::time::Duration;
use std::io::prelude::*;
use flate2::read::GzDecoder;
use futures::*;

pub struct DarwinProcessor {
    sess: Session,
    hdl: Handle,
    qn: String,
    timeout: Option<Timeout>,
    timeout_ms: u64,
}

impl Future for DarwinProcessor {
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

        while let Async::Ready(ev) = self.sess.poll()? {
            let ev = ev.unwrap();
            match ev {
                Connected => {
                    info!("Connected to Darwin, subscribing...");
                    self.timeout = None;
                    self.timeout_ms = 1000;
                    self.sess.subscription(&format!("/queue/{}", self.qn))
                        .start();
                },
                ErrorFrame(fr) => {
                    error!("Error frame, reconnecting: {:?}", fr);
                    self.sess.reconnect().unwrap();
                },
                Message { frame, .. } => {
                    let mut gz = GzDecoder::new(&frame.body as &[u8]);
                    let mut s = String::new();
                    match gz.read_to_string(&mut s) {
                        Ok(_) => {
                            info!("Got frame: {}", s);
                            match darwin_types::parse_pport_document(s.as_bytes()) {
                                Ok(doc) => info!("Parsed: {:?}", doc),
                                Err(e) => warn!("Failed to parse: {}", e)
                            }
                        },
                        Err(e) => {
                            warn!("Failed to deflate frame: {}", e);
                        }
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

fn main() {
    println!("osms-darwin starting");
    println!("Loading config...");
    let conf = DarwinConfig::make().unwrap();
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
    info!("Initialising session...");
    let mut core = Core::new().unwrap();
    let hdl = core.handle();
    let darwin_url = conf.darwin_url.as_ref().map(|x| x as &str).unwrap_or("datafeeds.nationalrail.co.uk");
    let sess = SessionBuilder::new(darwin_url, conf.darwin_port.unwrap_or(61613))
        .with(Credentials(&conf.username, &conf.password))
        .with(Header::new("client-id", "eta@theta.eu.org"))
        .with(HeartBeat(5_000, 2_000))
        .start(hdl.clone())
        .unwrap();
    info!("Running client...");
    let p = DarwinProcessor { sess, hdl, timeout: None, timeout_ms: 1000, qn: conf.queue_name };
    core.run(p).unwrap();
}
