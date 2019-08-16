//! Connects to various sources of live data, and relays information to tspl-zugfuhrer.

pub mod errors;
pub mod config;
pub mod nrod;
pub mod darwin;
pub mod conn;

use log::*;
use failure::format_err;
use tspl_util::ConfigExt;
use self::config::Config;
use crate::conn::{StompProcessor, DarwinConfig, NrodConfig};
use crate::nrod::NrodWorker;
use crate::darwin::DarwinWorker;
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
    if cfg.use_darwin && cfg.stomp_host.is_none() {
        Err(format_err!("stomp_host must be specified if darwin is used"))?
    }
    let variant = if cfg.use_darwin { "Darwin" } else { "NROD" };
    info!("initializing {} session", variant);
    let mut core = Core::new().unwrap();
    let hdl = core.handle();
    if cfg.use_darwin {
        let (tx, rx) = crossbeam_channel::unbounded();
        let dcfg = DarwinConfig {
            username: &cfg.username,
            password: &cfg.password,
            stomp_host: &cfg.stomp_host
                .as_ref().unwrap() as &str,
            stomp_port: cfg.stomp_port,
            queue_updates: cfg.darwin_queue_updates
                .as_ref().map(|x| x as &str)
        };
        let proc = StompProcessor::new_darwin(&dcfg, tx, hdl)?;
        info!("spawning {} worker thread(s)", cfg.n_threads);
        let cmap = Arc::new(CHashMap::new());
        for _ in 0..cfg.n_threads {
            let mut worker = DarwinWorker::new(rx.clone(), cmap.clone(), cfg.service_zugfuhrer.clone());
            thread::spawn(move || {
                worker.run();
            });
        }
        info!("tspl-nrod running in Darwin mode!");
        core.run(proc)?;
    }
    else {
        let (tx, rx) = crossbeam_channel::unbounded();
        let ncfg = NrodConfig {
            username: &cfg.username,
            password: &cfg.password,
            stomp_host: cfg.stomp_host.as_ref().map(|x| x as &str),
            stomp_port: cfg.stomp_port
        };
        let proc = StompProcessor::new_nrod(&ncfg, tx, hdl)?;
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
    }
    Ok(())
}
