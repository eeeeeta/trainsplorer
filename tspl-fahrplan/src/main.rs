//! Schedule storage component, storing and updating copies of CIF/ITPS schedules.

pub mod errors;
pub mod types;
pub mod import;
pub mod download;
pub mod dl_scheduler;
pub mod config;
pub mod proto;
pub mod ctx;

use tspl_sqlite::r2d2;
use tspl_util::ConfigExt;
use self::config::Config;
use tspl_proto::RpcListener;
use crate::proto::FahrplanRpc;
use crate::ctx::App;
use log::*;

fn main() -> errors::Result<()> {
    tspl_util::setup_logging()?;
    info!("tspl-fahrplan, but not yet");
    info!("loading config");
    let cfg = Config::load()?;
    info!("initializing db");
    let manager = tspl_sqlite::TsplConnectionManager::initialize(&cfg.database_path, &types::MIGRATIONS)?;
    let pool = r2d2::Pool::new(manager)?;
    info!("initializing update scheduler");
    let mut dls = dl_scheduler::UpdateScheduler::new(pool.clone(), &cfg)?;
    let dls_tx = dls.take_sender();
    dls.run().unwrap();
    info!("starting RPC listener");
    let mut listener: RpcListener<FahrplanRpc> = RpcListener::new(&cfg.listen_url)?;
    info!("listening for requests on: {}", cfg.listen_url);
    let mut app = App { pool, dls_tx };
    loop {
        let req = listener.recv()?;
        match req.decode() {
            Ok(v) => {
                info!("got request: {:?}", v);
                let ret = app.process_request(v)?;
                req.reply(ret)?;
            },
            Err(e) => {
                println!("decode error: {}", e);
            }
        }
    }
}
