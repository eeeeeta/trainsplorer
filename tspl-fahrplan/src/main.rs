//! Schedule storage component, storing and updating copies of CIF/ITPS schedules.

pub mod errors;
pub mod types;
pub mod config;
pub mod ctx;
pub mod updater;

use tspl_sqlite::r2d2;
use tspl_util::ConfigExt;
use self::config::Config;
use std::sync::{RwLock, Arc};
use crate::updater::DbUpdater;
use crate::ctx::App;
use log::*;

pub static DATABASE_PATH: &str = "./fahrplan.sqlite";
pub static DATABASE_PATH_DL: &str = "./fahrplan-dl.sqlite";
pub static DATABASE_PATH_TEMP: &str = "./fahrplan-temp.sqlite";

fn main() -> errors::Result<()> {
    tspl_util::setup_logging()?;
    info!("tspl-fahrplan, but not yet");
    info!("loading config");
    let cfg = Config::load()?;
    info!("initializing db");
    let manager = tspl_sqlite::TsplConnectionManager::initialize(DATABASE_PATH, &types::MIGRATIONS)?;
    let pool = r2d2::Pool::new(manager)?;
    let pool = Arc::new(RwLock::new(pool));
    info!("initializing db updater");
    let mut dbu = DbUpdater::init(&cfg, pool.clone())?;
    info!("downloading db from GCS");
    dbu.download(DATABASE_PATH_DL)?;
    info!("backgrounding updater");
    dbu.run_in_background();
    let app = Arc::new(App { pool });
    tspl_util::http::start_server(&cfg.listen_url, app);
}
