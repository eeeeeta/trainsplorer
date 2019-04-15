//! Schedule storage component, storing and updating copies of CIF/ITPS schedules.

pub mod errors;
pub mod types;
pub mod import;

use tspl_sqlite::initialize_db;
use std::io::BufReader;
use std::fs::File;
use log::*;

fn main() -> errors::Result<()> {
    tspl_util::setup_logging()?;
    info!("tspl-fahrplan, but not yet");
    info!("initializing db");
    let mut conn = initialize_db("./fahrplan.sqlite", &types::MIGRATIONS)?;
    info!("reading records from ./records.json");
    let f = File::open("./records.json")?;
    let f = BufReader::new(f);
    import::apply_schedule_records(&mut conn, f)?;
    Ok(())
}
