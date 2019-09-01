//! Downloading and importing CORPUS data.
//!
//! Shamelessly stolen from `tspl-zugfuhrer`, which should eventually
//! just be written to use the `tspl-nennen` data instead of importing
//! CORPUS data itself...

use tspl_sqlite::traits::*;
use log::*;
use ntrod_types::reference::CorpusData;
use tspl_util::nrod::NrodDownloader;

use crate::types::WrappedCorpusEntry;
use crate::errors::*;
use crate::config::Config;

pub struct CorpusDownloader<'a> {
    inner: NrodDownloader,
    db: &'a mut Connection
}
impl<'a> CorpusDownloader<'a> {
    pub fn new<'b>(db: &'a mut Connection, cfg: &'b Config) -> Self {
        let inner = NrodDownloader::new(cfg.username.clone(), cfg.password.clone(), cfg.base_url.clone());
        Self { inner, db }
    }
    pub fn import(&mut self) -> Result<()> {
        info!("Requesting CORPUS reference data");
        let resp = self.inner.download("/ntrod/SupportingFileAuthenticate?type=CORPUS")?;
        info!("Importing CORPUS reference data");
        let data: CorpusData = ::serde_json::from_reader(resp)?;
        let mut inserted = 0;
        let trans = self.db.transaction()?;
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
}

