//! Actually connecting to NROD, and reconnecting on failure.

use stomp::session::{SessionEvent, Session};
use stomp::header::Header;
use stomp::session_builder::SessionBuilder;
use stomp::subscription::AckOrNack;
use stomp::connection::*;
use tokio_core::reactor::{Timeout, Handle};
use crossbeam_channel::Sender;
use failure::Error;
use futures::{Poll, Async, Future, Stream};
use log::*;
use std::time::Duration;

use crate::worker::WorkerMessage;
use crate::config::Config;
use crate::errors::Result;

pub struct NrodProcessor {
    sess: Session,
    hdl: Handle,
    timeout: Option<Timeout>,
    timeout_ms: u64,
    tx: Sender<WorkerMessage>,
}
impl NrodProcessor {
    pub fn new(conf: &Config, tx: Sender<WorkerMessage>, hdl: Handle) -> Result<Self> {
        let stomp_host = conf.stomp_host
            .as_ref().map(|x| x as &str)
            .unwrap_or("datafeeds.networkrail.co.uk");
        let sess = SessionBuilder::new(stomp_host, conf.stomp_port.unwrap_or(61618))
            .with(Credentials(&conf.username, &conf.password))
            .with(Header::new("client-id", "eta@theta.eu.org"))
            .with(HeartBeat(5_000, 2_000))
            .start(hdl.clone())?;
        Ok(Self {
            sess, hdl, tx,
            timeout: None,
            timeout_ms: 1000
        })
    }
}
impl Future for NrodProcessor {
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
                    info!("Connected to NTROD; subscribing...");
                    self.sess.subscription("/topic/TRAIN_MVT_ALL_TOC")
                        .with(Header::new("activemq.subscriptionName", "tspl-nrod"))
                        .start();
                },
                ErrorFrame(fr) => {
                    error!("Error frame, reconnecting: {:?}", fr);
                    self.sess.disconnect();
                },
                Message { destination, frame, .. } => {
                        self.timeout = None;
                        self.timeout_ms = 1000;
                        debug!("Got a NTROD message addressed to {}", destination);
                        if destination == "/topic/TRAIN_MVT_ALL_TOC" {
                            let st = String::from_utf8_lossy(&frame.body);
                            self.tx.send(WorkerMessage::Movement(st.into()));
                        }
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
