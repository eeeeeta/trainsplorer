pub mod types;
use db::{GenericConnection, DbType, InsertableDbType};
use ntrod_types::schedule;
use ntrod_types::reference;
use serde_json;
use std::io::{Read, BufRead};
use self::types::*;
use errors::*;

pub fn make_schedule_ways<T: GenericConnection>(conn: &T) -> Result<()> {
    debug!("make_schedule_ways: getting schedules...");
    let scheds = Schedule::from_select(conn, "", &[])?;
    debug!("make_schedule_ways: {} schedules to update", scheds.len());
    for sched in scheds {
        let trans = conn.transaction()?;
        sched.make_ways(&trans)?;
        trans.commit()?;
    }
    debug!("make_schedule_ways: complete!");
    Ok(())
}
pub fn apply_schedule_records<T: GenericConnection, R: BufRead>(conn: &T, rdr: R, restrict_atoc: Option<&str>) -> Result<()> {
    debug!("apply_schedule_records: running...");
    let mut inserted = 0;
    let trans = conn.transaction()?;
    for line in rdr.lines() {
        let line = line?;
        let rec: ::std::result::Result<schedule::Record, _> = serde_json::from_str(&line);
        let rec = match rec {
            Ok(r) => r,
            Err(e) => {
                warn!("apply_schedule_records: error parsing: {}", e);
                debug!("apply_schedule_records: line was: {}", line);
                continue;
            }
        };
        match rec {
            schedule::Record::Schedule(rec) => {
                if let Some(atc) = restrict_atoc {
                    if !rec.atoc_code.as_ref().map(|x| x == atc).unwrap_or(false) {
                        continue;
                    }
                }
                Schedule::apply_rec(&trans, rec)?;
                inserted += 1;
            },
            schedule::Record::Timetable(rec) => {
                debug!("apply_schedule_records: this is a {}-type timetable from {} (ts: {})",
                       rec.classification, rec.owner, rec.timestamp);
            },
            _ => {}
        }
    }
    trans.commit()?;
    debug!("apply_schedule_record: applied {} entries", inserted);
    Ok(())
}
pub fn import_corpus<T: GenericConnection, R: Read>(conn: &T, rdr: R) -> Result<()> {
    debug!("import_corpus: loading data from file...");
    let data: reference::CorpusData = serde_json::from_reader(rdr)?;
    debug!("import_corpus: inserting data into database...");
    let mut inserted = 0;
    let trans = conn.transaction()?;
    for ent in data.tiploc_data {
        if ent.contains_data() {
            ent.insert_self(&trans)?;
            inserted += 1;
        }
    }
    trans.commit()?;
    debug!("import_corpus: inserted {} entries", inserted);
    Ok(())
}
