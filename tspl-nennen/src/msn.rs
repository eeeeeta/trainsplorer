//! Handles actually parsing and importing the MSN entries.

use std::io::prelude::*;
use std::io::BufReader;
use std::fs::File;
use tspl_sqlite::traits::*;
use atoc_msn;
use atoc_msn::types::MsnRecord;
use log::*;

use crate::errors::*;
use crate::types::WrappedMsnStation;

pub fn load_msn(conn: &mut Connection, path: &str) -> Result<()> {
    let file = File::open(path)?;
    let buf_reader = BufReader::new(file);
    let trans = conn.transaction()?;

    let mut recs = 0;
    for line in buf_reader.lines() {
        let line = line?;
        if let Ok((_, data)) = atoc_msn::msn_record(&line) {
            match data {
                MsnRecord::Header(h) => {
                    info!("MSN file creation timestamp: {}", h.timestamp); 
                },
                MsnRecord::Station(s) => {
                    let s = WrappedMsnStation(s);
                    s.insert_self(&trans)?;
                    recs += 1;
                },
                _ => {}
            }
        }
    }
    trans.commit()?;
    info!("inserted {} MSN station entries", recs);
    Ok(())
}
