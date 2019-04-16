//! Schedule storage component, storing and updating copies of CIF/ITPS schedules.

pub mod errors;
pub mod types;
pub mod import;
pub mod download;
pub mod config;

use tspl_util::ConfigExt;
use self::config::Config;
use self::download::{Downloader, JobType};
use log::*;

fn main() -> errors::Result<()> {
    tspl_util::setup_logging()?;
    info!("tspl-fahrplan, but not yet");
    info!("loading config");
    let cfg = Config::load()?;
    info!("initializing db");
    let mut conn = tspl_sqlite::initialize_db(&cfg.database_path, &types::MIGRATIONS)?;
    info!("testing importing a daily update");
    let mut dl = Downloader::new(&cfg);
    dl.do_job(&mut conn, JobType::Init)?;
    Ok(())
}
