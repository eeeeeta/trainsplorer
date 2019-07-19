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
use crate::ctx::App;
use std::sync::{Mutex, Arc};
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
    let app = Arc::new(App { pool, dls_tx: Mutex::new(dls_tx) });
    info!("starting HTTP server on {}", cfg.listen_url);
    rouille::start_server(&cfg.listen_url, move |req| {
        app.process_request(req)
    });
}
