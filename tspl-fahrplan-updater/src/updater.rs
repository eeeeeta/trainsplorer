//! Handles database updating, with retries in the event of failure.

use tspl_sqlite::rusqlite::Connection;
use log::*;
use std::thread;
use failure::format_err;

use crate::download::Downloader;
use crate::config::Config;
use crate::errors::*;

/// How many milliseconds to wait before retrying a failed update job.
/// (default value)
///
/// This value is doubled for every failure (yay, exponential backoff!)
static UPDATE_TIMEOUT_MS: u32 = 16000;
/// How many retries per update job (default value)
static UPDATE_RETRIES: u32 = 9;

pub struct DatabaseUpdater<'a> {
    inner: &'a mut Connection,
    dl: Downloader,
    update_timeout_ms: u32,
    update_retries: u32
}

impl<'a> DatabaseUpdater<'a> {
    pub fn new(conn: &'a mut Connection, cfg: &Config) -> Self {
        let dl = Downloader::new(cfg);
        Self {
            inner: conn,
            dl,
            update_timeout_ms: cfg.update_timeout_ms.unwrap_or(UPDATE_TIMEOUT_MS),
            update_retries: cfg.update_retries.unwrap_or(UPDATE_RETRIES),
        }
    }
    pub fn init(&mut self) -> Result<()> {
        self.dl.do_init(self.inner)?;
        Ok(())
    }
    pub fn update(&mut self) -> Result<()> {
        let mut cur = 0;
        let mut timeout_ms = self.update_timeout_ms;
        loop {
            info!("Attempting to download update (try #{})", cur+1); 
            match self.dl.do_update(self.inner) {
                Ok(_) => {
                    info!("Update successfully downloaded.");
                    break Ok(());
                },
                Err(e) => {
                    warn!("Failed to fetch update: {}", e);
                    cur += 1;
                    if cur > self.update_retries {
                        error!("Attempt to fetch timed out!");
                        break Err(format_err!("fetch attempt timed out"));
                    }
                    warn!("Retrying in {} ms...", timeout_ms);
                    thread::sleep(::std::time::Duration::from_millis(timeout_ms as _));
                    timeout_ms *= 2;
                }
            }
        }
    }
}
