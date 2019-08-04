//! Stores live and historic train running information, as well as handling activations.

pub mod errors;
pub mod config;
pub mod types;

use log::*;
use tspl_util::ConfigExt;
use tspl_sqlite::r2d2;
use self::config::Config;
use errors::Result;

fn main() -> Result<()> {
    tspl_util::setup_logging()?;
    info!("tspl-zugfuhrer, but not yet");
    info!("loading config");
    let cfg = Config::load()?;
    info!("initializing database");
    let manager = tspl_sqlite::TsplConnectionManager::initialize(&cfg.database_path, &types::MIGRATIONS)?;
    let pool = r2d2::Pool::new(manager)?;
    Ok(())
}
