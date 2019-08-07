//! Downloading and importing CORPUS data.
//!
//! FIXME: A lot of this is cribbed from `src/download.rs` from fahrplan.

use reqwest::Client;
use tspl_sqlite::traits::*;
use tspl_sqlite::TsplPool;
use std::io::BufReader;
use log::*;
use ntrod_types::reference::CorpusData;
use failure::bail;

use crate::types::WrappedCorpusEntry;
use crate::errors::*;
use crate::config::Config;

pub struct CorpusDownloader {
    username: String,
    password: String,
    base_url: String,
    cli: Client,
    pool: TsplPool
}
impl CorpusDownloader {
    pub fn new(pool: TsplPool, cfg: &Config) -> Self {
        let cli = reqwest::Client::new();
        Self {
            username: cfg.username.clone(),
            password: cfg.password.clone(),
            base_url: cfg.base_url.clone()
                .unwrap_or_else(|| "https://datafeeds.networkrail.co.uk".into()),
            cli, pool
        }
    }
    pub fn import(&mut self) -> Result<()> {
        let mut db = self.pool.get()?;
        info!("Requesting CORPUS reference data");
        let url = format!("{}/ntrod/SupportingFileAuthenticate?type=CORPUS", self.base_url);
        let resp = self.cli.get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()?;
        let st = resp.status();
        if !st.is_success() {
            bail!("Response code {}: {}", st.as_u16(), st.canonical_reason().unwrap_or("unknown"));
        }
        let resp = BufReader::new(resp);
        info!("Importing CORPUS reference data");
        let data: CorpusData = ::serde_json::from_reader(resp)?;
        let mut inserted = 0;
        let trans = db.transaction()?;
        for ent in data.tiploc_data {
            if ent.contains_data() {
                let ent = WrappedCorpusEntry(ent);
                ent.insert_self(&trans)?;
                inserted += 1;
            }
        }
        trans.commit()?;
        info!("Imported {} CORPUS entries", inserted);
        Ok(())
    }
    pub fn should_import(&mut self) -> Result<bool> {
        let db = self.pool.get()?;
        let rows: i64 = db.query_row("SELECT COUNT(*) FROM corpus_entries", params![], |row| row.get(0))?;
        Ok(rows > 0)
    }
}

