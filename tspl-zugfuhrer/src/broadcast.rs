//! TCP server component for broadcasting live train updates.

use crossbeam_channel::{Receiver, Sender, select};
use std::net::{TcpListener, TcpStream};
use std::thread;
use tspl_sqlite::TsplPool;
use tspl_sqlite::traits::*;
use serde_derive::Serialize;
use chrono::prelude::*;
use log::*;

use crate::types::*;
use crate::errors::*;

#[derive(Clone)]
pub struct BroadcastSender(pub Sender<BroadcastPacket>);

impl BroadcastSender {
    pub fn send_update(&self, upd: BroadcastUpdate) {
        let now = Utc::now().naive_utc();
        let pkt = BroadcastPacket {
            ts: now,
            update: upd
        };
        let _ = self.0.send(pkt);
    }
    pub fn send_mvt(&self, mvt: TrainMvt) {
        self.send_update(BroadcastUpdate::Movement(mvt))
    }
    pub fn send_activation(&self, mvt: Train) {
        self.send_update(BroadcastUpdate::Activation(mvt))
    }
}

#[derive(Clone, Debug, Serialize)]
pub enum BroadcastUpdate {
    Activation(Train),
    SyncTrain(Train),
    Movement(TrainMvt),
}

#[derive(Clone, Debug, Serialize)]
pub struct BroadcastPacket {
    ts: NaiveDateTime,
    update: BroadcastUpdate
}

pub struct LiveTcpListener {
    inner: TcpListener,
    tx: Sender<TcpStream>,
}
impl LiveTcpListener {
    fn run(self) -> Result<()> {
        for stream in self.inner.incoming() {
            let stream = match stream {
                Ok(s) => s,
                Err(e) => {
                    warn!("Error accepting client: {}", e);
                    continue;
                }
            };
            self.tx.send(stream)?;
        }
        Ok(())
    }
}

pub struct LiveHandler {
    inner: TcpStream,
    rx: Receiver<BroadcastPacket>,
    pool: TsplPool
}
impl LiveHandler {
    fn send(&mut self, upd: &BroadcastPacket) -> Result<()> {
        serde_json::to_writer(&mut self.inner, upd)?;
        Ok(())
    }
    fn run(mut self) -> Result<()> {
        let addr = self.inner.peer_addr()?;
        info!("Synchronizing trains for live connection {}", addr);
        let db = self.pool.get()?;
        let today = Local::now().naive_utc().date();
        let trains = Train::from_select(&db, "WHERE date = ? AND terminated != false", &[&today])?;
        let len = trains.len();
        for train in trains {
            let now = Utc::now().naive_utc();
            let pkt = BroadcastPacket {
                ts: now,
                update: BroadcastUpdate::SyncTrain(train)
            };
            self.send(&pkt)?;
        }
        info!("Synchronized {} running trains with live connection {}", len, addr);
        loop {
            let msg = self.rx.recv()?;
            self.send(&msg)?;
        }
    }
}

pub struct LiveBroadcaster {
    streams: Vec<Sender<BroadcastPacket>>,
    pool: TsplPool,
    new_streams: Receiver<TcpStream>,
    updates: Receiver<BroadcastPacket>,
}

impl LiveBroadcaster {
    pub fn setup(pool: TsplPool, upd: Receiver<BroadcastPacket>, listen_url: &str) -> Result<()> {
        let listener = TcpListener::bind(listen_url)?;
        let (tx, rx) = crossbeam_channel::unbounded();
        let ltl = LiveTcpListener {
            inner: listener,
            tx
        };
        thread::spawn(move || {
            if let Err(e) = ltl.run() {
                error!("Live TCP listener failed: {}", e);
            }
        });
        let mut selfish = Self {
            streams: vec![],
            pool,
            new_streams: rx,
            updates: upd
        };
        thread::spawn(move || {
            if let Err(e) = selfish.run() {
                error!("LiveBroadcaster failed: {}", e);
            }
        });
        Ok(())
    }
    fn run(&mut self) -> Result<()> {
        loop {
            select! {
                recv(self.new_streams) -> msg => {
                    let msg = msg?;
                    let addr = msg.peer_addr()?;
                    let (tx, rx) = crossbeam_channel::unbounded();
                    let lh = LiveHandler {
                        inner: msg,
                        rx,
                        pool: self.pool.clone()
                    };
                    self.streams.push(tx);
                    thread::spawn(move || {
                        if let Err(e) = lh.run() {
                            warn!("Handler for {} failed: {}", addr, e);
                        }
                    });
                },
                recv(self.updates) -> msg => {
                    let msg = msg?;
                    let mut to_remove = vec![];
                    for (i, tx) in self.streams.iter_mut().enumerate() {
                        if let Err(_) = tx.send(msg.clone()) {
                            to_remove.push(i);
                        }
                    }
                    for idx in to_remove.into_iter().rev() {
                        self.streams.remove(idx);
                    }
                }
            }
        }
    }
}
