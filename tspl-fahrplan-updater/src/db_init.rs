//! Handles initializing the database, and creating a new one if the current one is somehow broken.

use tspl_sqlite::rusqlite::{Connection, NO_PARAMS};
use tspl_sqlite::traits::*;
use tspl_fahrplan::types as fpt;
use failure::format_err;
use chrono::prelude::*;
use std::path::Path;
use log::*;

use crate::errors::*;

/// Use tspl-sqlite to initialize the database at `path`.
fn sqlite_initialize_db(path: &str) -> Result<Connection> {
    let ret = tspl_sqlite::initialize_db(path, &fpt::MIGRATIONS)?;
    Ok(ret)
}
/// Check that the given database contains a valid collection of
/// imported schedule files.
///
/// If this check is successful, the database can be updated
/// instead of reinitialized from scratch.
///
/// This check fails if:
/// - there are no schedule files in the database
/// - there's a gap in the sequence numbers of the schedule files
/// - the last schedule file's timestamp isn't yesterday's date
fn check_schedule_files(conn: &mut Connection) -> Result<bool> {
    let files = fpt::ScheduleFile::from_select(&conn, "", NO_PARAMS)?;
    if files.len() == 0 {
        warn!("Rejecting database: no imported schedule files");
        return Ok(false);
    }
    let mut last = 0;
    let mut ts = NaiveDateTime::from_timestamp(0, 0);
    for file in files {
        if last != 0 {
            if file.sequence.saturating_sub(1) != last {
                warn!("Rejecting database: discontinuity between files {} and {}", file.sequence, last);
                return Ok(false);
            }
        }
        last = file.sequence;
        ts = match NaiveDateTime::from_timestamp_opt(file.timestamp as _, 0) {
            Some(t) => t,
            None => {
                warn!("Rejecting database: invalid ts {}", file.timestamp);
                return Ok(false);
            }
        }
    }
    let now = Utc::now().naive_utc();
    let yest = now.date().pred();
    if ts.date() != yest {
        warn!("Rejecting database: yesterday was {}, but last timestamp was {}", yest, ts);
        return Ok(false);
    }
    info!("Successfully validated previous database file!");
    Ok(true)
}
/// Try to load an existing database at `path`.
fn try_load_existing(path: &str) -> Result<Connection> {
    let mut db = sqlite_initialize_db(path)?;
    if !check_schedule_files(&mut db)? {
        Err(format_err!("failed schedule files check"))
    }
    else {
        Ok(db)
    }
}
/// Load a database from the given path, or (re)create one if the database is broken or corrupted.
///
/// Returns the database connection, and a boolean indicating whether the database can be updated,
/// or whether it must be reinitialized.
pub fn load_or_create_db(path: &str) -> Result<(Connection, bool)> {
    if Path::new(path).exists() {
        info!("Trying to load existing database at {}...", path);
        match try_load_existing(path) {
            Ok(db) => return Ok((db, true)),
            Err(e) => {
                warn!("Failed to load existing database: {}", e);
                warn!("Deleting the file...");
                std::fs::remove_file(path)?;
            }
        }
    }
    info!("Initializing database at {}...", path);
    let ret = sqlite_initialize_db(path)?;
    Ok((ret, false))
}
