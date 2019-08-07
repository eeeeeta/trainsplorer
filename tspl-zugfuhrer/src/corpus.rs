//! Downloading and importing CORPUS data.

use tspl_sqlite::traits::*;
use tspl_sqlite::TsplPool;
use log::*;
use ntrod_types::reference::CorpusData;
use tspl_util::nrod::NrodDownloader;

use crate::types::WrappedCorpusEntry;
use crate::errors::*;
use crate::config::Config;

pub struct CorpusDownloader {
    inner: NrodDownloader,
    pool: TsplPool
}
impl CorpusDownloader {
    pub fn new(pool: TsplPool, cfg: &Config) -> Self {
        let inner = NrodDownloader::new(cfg.username.clone(), cfg.password.clone(), cfg.base_url.clone());
        Self { inner, pool }
    }
    pub fn import(&mut self) -> Result<()> {
        let mut db = self.pool.get()?;
        info!("Requesting CORPUS reference data");
        let resp = self.inner.download("/ntrod/SupportingFileAuthenticate?type=CORPUS")?;
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
        Ok(rows == 0)
    }
}

