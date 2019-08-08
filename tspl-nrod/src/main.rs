//! Connects to the NROD TRUST Train Movements system.

pub mod errors;
pub mod config;
pub mod worker;
pub mod conn;

use log::*;
use tspl_util::ConfigExt;
use self::config::Config;
use crate::conn::NrodProcessor;
use crate::worker::NrodWorker;
use tokio_core::reactor::Core;
use chashmap::CHashMap;
use std::sync::Arc;
use std::thread;
use errors::Result;

fn main() -> Result<()> {
    tspl_util::setup_logging()?;
    info!("tspl-nrod, but not yet");
    info!("loading config");
    let cfg = Config::load()?;
    info!("initializing NROD session");
    let mut core = Core::new().unwrap();
    let hdl = core.handle();
    let (tx, rx) = crossbeam_channel::unbounded();
    let proc = NrodProcessor::new(&cfg, tx, hdl)?;
    info!("spawning {} worker thread(s)", cfg.n_threads);
    let cmap = Arc::new(CHashMap::new());
    for _ in 0..cfg.n_threads {
        let mut worker = NrodWorker::new(rx.clone(), cmap.clone(), cfg.service_zugfuhrer.clone());
        thread::spawn(move || {
            worker.run();
        });
    }
    info!("tspl-nrod running!");
    core.run(proc)?;
    Ok(())
}
