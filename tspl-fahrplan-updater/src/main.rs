//! Downloads and updates the tspl-fahrplan ITPS schedules.

pub mod errors;
pub mod import;
pub mod config;
pub mod download;
pub mod db_init;
pub mod updater;

use crate::config::Config;
use crate::updater::DatabaseUpdater;
use tspl_util::ConfigExt;
use tspl_gcs::CloudStorage;
use tspl_gcs::errors::GcsError;
use std::path::Path;
use log::*;

static CURRENT_OBJ: &str = "current.sqlite";
static CURRENT_PATH: &str = "./current.sqlite";
static BACKUP_PATH: &str = "./backup.sqlite";

fn main() -> errors::Result<()> {
    tspl_util::setup_logging()?;
    info!("tspl-zugfuhrer-updater, but not yet");
    info!("loading config");
    let cfg = Config::load()?;
    info!("initialising GCS (bucket = {})", cfg.bucket_name);
    let mut gcs = CloudStorage::init(cfg.bucket_name.clone(), &cfg.service_account_key_path)?;
    if !Path::new(BACKUP_PATH).exists() {
        info!("attempting to download current database...");
        match gcs.download_object(CURRENT_OBJ, CURRENT_PATH) {
            Ok(_) => info!("download successful"),
            Err(GcsError::NotFound) => warn!("current database not found"),
            Err(e) => {
                error!("download failed: {}", e);
                Err(e)?
            }
        }
        info!("opening database");
        let (mut db, update) = db_init::load_or_create_db(CURRENT_PATH)?;
        let mut updater = DatabaseUpdater::new(&mut db, &cfg);
        if update {
            updater.update()?;
        }
        else {
            info!("downloading full schedule file...");
            updater.init()?;
        }
        info!("vacuuming into backup file...");
        db.execute_batch(&format!("VACUUM INTO '{}';", BACKUP_PATH))?;
    }
    else {
        warn!("backup file exists already; just uploading");
    }
    info!("uploading vacuumed backup file...");
    gcs.upload_object(BACKUP_PATH, CURRENT_OBJ)?;
    info!("done!");
    Ok(())
}
