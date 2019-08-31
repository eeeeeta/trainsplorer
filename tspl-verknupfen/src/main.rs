//! Queries `tspl-zugfuhrer` and `tspl-fahrplan` for movement information, and combines the two
//! responses into a summary.

pub mod errors;
pub mod config;
pub mod ctx;
pub mod types;

use log::*;
use tspl_util::ConfigExt;
use self::config::Config;
use self::ctx::App;
use errors::Result;

fn main() -> Result<()> {
    tspl_util::setup_logging()?;
    info!("tspl-verknupfen, but not yet");
    info!("loading config");
    let cfg = Config::load()?;
    let app = App::new(&cfg);
    tspl_util::http::start_server(&cfg.listen, app);
}
