//! Downloads CORPUS and MSN data, then uploads a database containing station
//! and other reference information.

pub mod errors;
pub mod config;
pub mod types;
pub mod msn;
pub mod corpus;

use crate::config::Config;
use crate::corpus::CorpusDownloader;
use crate::types::*;
use tspl_sqlite::traits::*;
use tspl_util::ConfigExt;
use tspl_gcs::CloudStorage;
use log::*;

static MSN_FILE_OBJ: &str = "names.msn";
static CURRENT_OBJ: &str = "refdata.sqlite";

fn main() -> errors::Result<()> {
    tspl_util::setup_logging()?;
    info!("tspl-nennen, but not yet");
    info!("loading config");
    let cfg = Config::load()?;
    info!("initialising GCS (bucket = {})", cfg.bucket_name);
    let mut gcs = CloudStorage::init(cfg.bucket_name.clone(), &cfg.service_account_key_path)?;
    info!("initialising database");
    let mut db = tspl_sqlite::initialize_db("./current.sqlite", &types::MIGRATIONS)?;

    info!("downloading Master Station Names file...");
    gcs.download_object(MSN_FILE_OBJ, "./names.msn")?;
    info!("loading MSN entries");
    msn::load_msn(&mut db, "./names.msn")?;
    info!("loading CORPUS entries");
    let mut cd = CorpusDownloader::new(&mut db, &cfg);
    cd.import()?;

    info!("generating station names");
    for msn in WrappedMsnStation::from_select(&db, "", NO_PARAMS)? {
        StationName {
            id: -1,
            name: titlecase::titlecase(&msn.0.name),
            crs: Some(msn.0.crs.clone()),
            tiploc: None
        }.insert_self(&db)?;
        if msn.0.subsidiary_crs != msn.0.crs {
            StationName {
                id: -1,
                name: titlecase::titlecase(&msn.0.name),
                crs: Some(msn.0.subsidiary_crs),
                tiploc: None
            }.insert_self(&db)?;
        }
        StationName {
            id: -1,
            name: titlecase::titlecase(&msn.0.name),
            crs: None,
            tiploc: Some(msn.0.tiploc)
        }.insert_self(&db)?;
    }
    for corp in WrappedCorpusEntry::from_select(&db, "WHERE nlcdesc IS NOT NULL AND (tiploc IS NOT NULL) or (crs IS NOT NULL)", NO_PARAMS)? {
        let nlcdesc = corp.0.nlcdesc.unwrap();
        if let Some(tpl) = corp.0.tiploc {
            StationName {
                id: -1,
                name: titlecase::titlecase(&nlcdesc),
                crs: None,
                tiploc: Some(tpl)
            }.insert_self(&db)?;
        }
        if let Some(crs) = corp.0.crs {
            StationName {
                id: -1,
                name: titlecase::titlecase(&nlcdesc),
                crs: Some(crs),
                tiploc: None
            }.insert_self(&db)?;
        }
    }

    info!("vacuuming into backup file...");
    db.execute_batch("VACUUM INTO './backup.sqlite';")?;
    info!("uploading vacuumed backup file...");
    gcs.upload_object("./backup.sqlite", CURRENT_OBJ)?;
    info!("done!");
    Ok(())
}
