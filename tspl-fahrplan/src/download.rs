//! Utilities for downloading schedule files from NROD.

use reqwest::{Response, Client};
use failure::bail;
use chrono::*;
use crate::errors::*;
use log::*;
use flate2::bufread::GzDecoder;
use std::io::BufReader;
use tspl_sqlite::traits::*;
use crate::import;
use crate::config::Config;
use serde_derive::{Serialize, Deserialize};

pub struct Downloader {
    username: String,
    password: String,
    base_url: String,
    cli: Client,
}
#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub enum JobType {
    Init,
    Recover,
    Update
}

// FIXME(perf): Yes, two layers of buffering.
//
// - The first layer of buffering is to avoid copious syscalls
//   when reading from the network. Not sure whether it's really needed.
// - The second layer of buffering is required by `apply_schedule_records`,
//   which needs to call read_line().
//
// I really don't know how performant this is. My guess is "not very"???
type ResponseReader = BufReader<GzDecoder<BufReader<Response>>>;

impl Downloader {
    pub fn new(cfg: &Config) -> Self {
        let cli = reqwest::Client::new();
        Self {
            username: cfg.username.clone(),
            password: cfg.password.clone(),
            base_url: cfg.base_url.clone()
                .unwrap_or_else(|| "https://datafeeds.networkrail.co.uk".into()),
            cli
        }
    }
    fn download(&mut self, url: &str) -> Result<ResponseReader> {
        debug!("Requesting: {}", url);
        let resp = self.cli.get(url)
            .basic_auth(&self.username, Some(&self.password))
            .send()?;
        let st = resp.status();
        if !st.is_success() {
            bail!("Response code {}: {}", st.as_u16(), st.canonical_reason().unwrap_or("unknown"));
        }
        let resp = BufReader::new(GzDecoder::new(BufReader::new(resp)));
        Ok(resp)
    }
    fn do_full(&mut self, conn: &mut Connection) -> Result<()> {
        let data = self.download(
            &format!("{}/ntrod/CifFileAuthenticate?type=CIF_ALL_FULL_DAILY&day=toc-full",
                     self.base_url))?;
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
        let data = self.download(
            &format!("{}/ntrod/CifFileAuthenticate?type=CIF_ALL_UPDATE_DAILY&day={}",
                     self.base_url, file))?;
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
