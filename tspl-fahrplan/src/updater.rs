//! Handles downloading schedule files from GCS, including updated versions when they become
//! available.

use tspl_gcs::CloudStorage;
use tspl_gcs::errors::GcsError;
use tspl_sqlite::{r2d2, TsplPool};
use failure::format_err;
use log::*;
use std::thread;
use std::time::Duration;
use std::fs;
use std::sync::{RwLock, Arc};

use crate::errors::*;
use crate::config::Config;
use crate::*;

pub static OBJECT_NAME: &str = "current.sqlite";
pub static GCS_CHECK_SECS: u32 = 300;

pub struct DbUpdater {
    inner: CloudStorage,
    object_name: String,
    last_updated: String,
    sleep_dur: Duration,
    pool: Arc<RwLock<TsplPool>>
}

impl DbUpdater {
    pub fn init(cfg: &Config, pool: Arc<RwLock<TsplPool>>) -> Result<Self> {
        let mut inner = CloudStorage::init(cfg.bucket_name.clone(), &cfg.service_account_key_path)?;
        let object_name = cfg.object_name.clone().unwrap_or(OBJECT_NAME.into());
        info!("Downloading initial metadata for '{}'", object_name);
        let obj = inner.get_object(&object_name)?;
        let last_updated = obj.updated
            .ok_or(format_err!("no updated ts"))?;
        let gcs_check_secs = cfg.gcs_check_secs.unwrap_or(GCS_CHECK_SECS);
        let sleep_dur = Duration::new(gcs_check_secs as _, 0);
        Ok(Self { inner, object_name, last_updated, sleep_dur, pool })
    }
    pub fn run_in_background(self) {
        thread::spawn(move || {
            self.run().unwrap();
        });
    }
    pub fn download(&mut self, path: &str) -> Result<()> {
        self.inner.download_object(&self.object_name, path)?;
        info!("Recreating database pool...");
        // Get an exclusive lock on the database pool, so
        // we can overwrite its file.
        let mut lock = self.pool.write().unwrap();
        info!("Acquired lock");
        // Make a dummy temporary database.
        let manager = tspl_sqlite::TsplConnectionManager::initialize(DATABASE_PATH_TEMP, &types::MIGRATIONS)?;
        let pool = r2d2::Pool::new(manager)?;
        // Overwrite the existing pool to close the file.
        *lock = pool;
        // Rename the downloaded object, overwriting the previous
        // file.
        fs::rename(path, DATABASE_PATH)?;
        // Load the f'real database.
        let manager = tspl_sqlite::TsplConnectionManager::initialize(DATABASE_PATH, &types::MIGRATIONS)?;
        let pool = r2d2::Pool::new(manager)?;
        *lock = pool;
        // We're now done here!
        Ok(())
    }
    pub fn check_if_changed(&mut self) -> Result<bool> {
        let obj = match self.inner.get_object(&self.object_name) {
            Ok(o) => o,
            Err(GcsError::NotFound) => {
                warn!("Object seems to have disappeared! Ignoring for now...");
                return Ok(false)
            },
            Err(e) => Err(e)?
        };

        let last_updated = obj.updated
            .ok_or(format_err!("no updated ts"))?;
        if last_updated != self.last_updated {
            info!("New file, updated at: {}", last_updated);
            self.last_updated = last_updated;
            Ok(true)
        }
        else {
            Ok(false)
        }
    }
    pub fn run(mut self) -> Result<()> {
        loop {
            if self.check_if_changed()? {
                info!("Downloading changed file to '{}'...", DATABASE_PATH_DL);
                self.download(DATABASE_PATH_DL)?;
            }
            thread::sleep(self.sleep_dur);
        }
    }
}
