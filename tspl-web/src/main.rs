//! Provides a nice snazzy web frontend for the whole project.

pub mod errors;
pub mod config;
pub mod ctx;
pub mod tmpl;
pub mod templates;
pub mod suggestions;

use log::*;
use tspl_util::ConfigExt;
use tspl_sqlite::r2d2;
use tspl_gcs::CloudStorage;
use std::sync::Arc;

use crate::config::Config;
use crate::ctx::App;
use crate::errors::*;

fn main() -> Result<()> {
    tspl_util::setup_logging()?;
    info!("tspl-web, but not yet");
    info!("loading config");
    let cfg = Config::load()?;
    info!("initialising Handlebars");
    let hbs = tmpl::handlebars_init()?;
    info!("initialising GCS (bucket = {})", cfg.bucket_name);
    let mut gcs = CloudStorage::init(cfg.bucket_name.clone(), &cfg.service_account_key_path)?;
    info!("downloading tspl-nennen data...");
    gcs.download_object("refdata.sqlite", "./refdata.sqlite")?;
    info!("initialising reference database");
    let manager = tspl_sqlite::TsplConnectionManager::initialize("./refdata.sqlite", &tspl_nennen::types::MIGRATIONS)?;
    let pool = r2d2::Pool::new(manager)?;
    let srv = Arc::new(App::new(&cfg, pool, hbs));
    let listen_url = &cfg.listen;
    info!("Starting HTTP server on {}", listen_url);
    rouille::start_server(listen_url, move |req| {
        srv.handle_request(req)
    })
}
