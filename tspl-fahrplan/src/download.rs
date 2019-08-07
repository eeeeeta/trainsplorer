//! Utilities for downloading schedule files from NROD.

use chrono::*;
use crate::errors::*;
use log::*;
use tspl_sqlite::traits::*;
use crate::import;
use crate::config::Config;
use serde_derive::{Serialize, Deserialize};
use std::str::FromStr;
use tspl_util::nrod::NrodDownloader;

pub struct Downloader {
    inner: NrodDownloader
}
#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub enum JobType {
    Init,
    Recover,
    Update
}

impl FromStr for JobType {
    type Err = ();

    fn from_str(s: &str) -> ::std::result::Result<Self, ()> {
        match s {
            "init" => Ok(JobType::Init),
            "recover" => Ok(JobType::Recover),
            "update" => Ok(JobType::Update),
            _ => Err(())
        }
    }
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
    pub fn do_recover(&mut self, conn: &mut Connection) -> Result<()> {
        info!("Recovering after a skipped update: reinitializing...");
        self.do_full(conn)?;
        let metaseq: u32 = conn.query_row(
            "SELECT sequence FROM schedule_files ORDER BY sequence DESC LIMIT 1",
            NO_PARAMS,
            |row| row.get(0))?;
        info!("Deleting stale schedules (metaseq != {})...", metaseq);
        let del = conn.execute("DELETE FROM schedules WHERE file_metaseq != ?",
                               params![metaseq])?;
        info!("Deleted {} schedules.", del);
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
    pub fn do_job(&mut self, conn: &mut Connection, ty: JobType) -> Result<()> {
        match ty {
            JobType::Init => self.do_init(conn)?,
            JobType::Recover => self.do_recover(conn)?,
            JobType::Update => self.do_update(conn)?,
        }
        Ok(())
    }
}
