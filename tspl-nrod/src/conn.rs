//! Actually connecting to a STOMP message queue, and reconnecting on failure.

use stomp::session::{SessionEvent, Session};
use stomp::header::Header;
use stomp::frame::Frame;
use stomp::session_builder::SessionBuilder;
use stomp::subscription::AckOrNack;
use stomp::connection::*;
use tokio_core::reactor::{Timeout, Handle};
use crossbeam_channel::Sender;
use failure::Error;
use futures::{Poll, Async, Future, Stream};
use flate2::bufread::GzDecoder;
use std::io::prelude::*;
use log::*;
use std::time::Duration;

use crate::nrod::NrodMessage;
use crate::darwin::DarwinMessage;
use crate::errors::Result;

pub trait StompType {
    fn on_connect(&mut self, sess: &mut Session) -> Result<()>;
    fn on_message(&mut self, dest: &str, frame: &Frame) -> Result<()>; 
}

pub struct Nrod {
    tx: Sender<NrodMessage>
}
impl StompType for Nrod {
    fn on_connect(&mut self, sess: &mut Session) -> Result<()> {
        info!("Connected to NTROD; subscribing...");
        sess.subscription("/topic/TRAIN_MVT_ALL_TOC")
            .with(Header::new("activemq.subscriptionName", "tspl-nrod"))
            .start();
        Ok(())
    }
    fn on_message(&mut self, dest: &str, frame: &Frame) -> Result<()> {
        debug!("Got a NTROD message addressed to {}", dest);
        if dest == "/topic/TRAIN_MVT_ALL_TOC" {
            let st = String::from_utf8_lossy(&frame.body);
            self.tx.send(NrodMessage::Movement(st.into()));
        }
        Ok(())
    }
}
pub struct Darwin {
    tx: Sender<DarwinMessage>,
    queue_updates: Option<String>
}
impl Darwin {
    fn updates_name(&self) -> &str {
        self.queue_updates
            .as_ref().map(|x| x as &str)
            .unwrap_or("darwin.pushport-v16")
    }
}
impl StompType for Darwin {
    fn on_connect(&mut self, sess: &mut Session) -> Result<()> {
        info!("Connected to Darwin; subscribing..");
        let name = self.updates_name();
        sess.subscription(name)
            .with(Header::new("activemq.subscriptionName", "tspl-darwin"))
            .start();
        Ok(())
    }
    fn on_message(&mut self, dest: &str, frame: &Frame) -> Result<()> {
        debug!("Got a Darwin message addressed to {}", dest);
        if dest == self.updates_name() {
            let mut gz = GzDecoder::new(&frame.body as &[u8]);
            let mut st = String::new();
            if let Err(e) = gz.read_to_string(&mut st) {
                error!("Failed to deflate frame: {}", e);
                return Ok(());
            }
            self.tx.send(DarwinMessage::Pport(st));
        }
        Ok(())
    }
}
pub struct NrodConfig<'a> {
    pub username: &'a str,
    pub password: &'a str,
    pub stomp_host: Option<&'a str>,
    pub stomp_port: Option<u16>
}
pub struct DarwinConfig<'a> {
    pub username: &'a str,
    pub password: &'a str,
    pub stomp_host: &'a str,
    pub stomp_port: Option<u16>,
    pub queue_updates: Option<&'a str>
}
pub struct StompProcessor<T> {
    sess: Session,
    hdl: Handle,
    timeout: Option<Timeout>,
    timeout_ms: u64,
    inner: T
}
impl StompProcessor<Nrod> {
    pub fn new_nrod(conf: &NrodConfig, tx: Sender<NrodMessage>, hdl: Handle) -> Result<StompProcessor<Nrod>> {
        let stomp_host = conf.stomp_host
            .unwrap_or("datafeeds.networkrail.co.uk");
        let sess = SessionBuilder::new(stomp_host, conf.stomp_port.unwrap_or(61618))
            .with(Credentials(conf.username, conf.password))
            .with(Header::new("client-id", "eta@theta.eu.org"))
            .with(HeartBeat(5_000, 2_000))
            .start(hdl.clone())?;
        let inner = Nrod { tx };
        Ok(StompProcessor {
            sess, hdl, inner,
            timeout: None,
            timeout_ms: 1000
        })
    }
}
impl StompProcessor<Darwin> {
    pub fn new_darwin(conf: &DarwinConfig, tx: Sender<DarwinMessage>, hdl: Handle) -> Result<StompProcessor<Darwin>> {
        let stomp_host = conf.stomp_host;
        let sess = SessionBuilder::new(stomp_host, conf.stomp_port.unwrap_or(61613))
            .with(Credentials(conf.username, conf.password))
            .with(Header::new("client-id", "eta@theta.eu.org"))
            .with(HeartBeat(5_000, 2_000))
            .start(hdl.clone())?;
        let inner = Darwin { 
            tx,
            queue_updates: conf.queue_updates
                .map(|x| x.into())
        };
        Ok(StompProcessor {
            sess, hdl, inner,
            timeout: None,
            timeout_ms: 1000
        })
    }
}
impl<T> Future for StompProcessor<T> where T: StompType {
    type Item = ();
    type Error = Error;

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
                    self.inner.on_connect(&mut self.sess)?;
                },
                ErrorFrame(fr) => {
                    error!("Error frame, reconnecting: {:?}", fr);
                    self.sess.disconnect();
                },
                Message { destination, frame, .. } => {
                        self.timeout = None;
                        self.timeout_ms = 1000;
                        self.inner.on_message(&destination, &frame)?;
                        self.sess.acknowledge_frame(&frame, AckOrNack::Ack);
                },
                Disconnected(reason) => {
                    error!("Disconnected: {:?}", reason);
                    error!("Reconnecting in {} ms", self.timeout_ms);
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
