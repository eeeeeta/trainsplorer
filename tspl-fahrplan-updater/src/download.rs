//! Utilities for downloading schedule files from NROD.

use chrono::*;
use crate::errors::*;
use log::*;
use tspl_sqlite::traits::*;
use crate::import;
use crate::config::Config;
use tspl_util::nrod::NrodDownloader;

pub struct Downloader {
    inner: NrodDownloader
}

impl Downloader {
    pub fn new(cfg: &Config) -> Self {
        let inner = NrodDownloader::new(cfg.username.clone(), cfg.password.clone(), cfg.base_url.clone());
        Self { inner }
    }
    fn do_full(&mut self, conn: &mut Connection) -> Result<()> {
        let data = self.inner.download("/ntrod/CifFileAuthenticate?type=CIF_ALL_FULL_DAILY&day=toc-full")?;
        import::apply_schedule_records(conn, data)?;
        Ok(())
    }
    pub fn do_init(&mut self, conn: &mut Connection) -> Result<()> {
        info!("Initializing schedule database...");
        self.do_full(conn)?;
        Ok(())
    }
    pub fn do_update(&mut self, conn: &mut Connection) -> Result<()> {
        let time = Utc::now();
        let weekd = time.weekday().pred();
        let file = match weekd {
            Weekday::Mon => "toc-update-mon",
            Weekday::Tue => "toc-update-tue",
            Weekday::Wed => "toc-update-wed",
            Weekday::Thu => "toc-update-thu",
            Weekday::Fri => "toc-update-fri",
            Weekday::Sat => "toc-update-sat",
            Weekday::Sun => "toc-update-sun",
        };
        info!("Fetching update file {}...", file);
        let data = self.inner.download(
            &format!("/ntrod/CifFileAuthenticate?type=CIF_ALL_UPDATE_DAILY&day={}",
                     file))?;
        import::apply_schedule_records(conn, data)?;
        Ok(())
    }
}
