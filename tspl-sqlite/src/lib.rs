//! The tspl-sqlite crate provides a common set of functions
//! for initializing, migrating and managing SQLite databases.
//!
//! It also includes some traits used to make common DB operations
//! (such as SELECT) easier.

pub mod errors;
pub mod traits;
pub mod migrations;

use self::errors::Result;
use rusqlite::Connection;
use self::migrations::Migration;
use std::path::Path;

pub fn initialize_db<P: AsRef<Path>>(path: P, migrations: &[Migration]) -> Result<Connection> {
    let mut conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    conn.execute_batch("PRAGMA journal_mode = WAL;")?;
    migrations::initialize_migrations(&mut conn)?;
    migrations::run_pending_migrations(&mut conn, migrations)?;
    Ok(conn)
}

pub use rusqlite;
pub use uuid;
