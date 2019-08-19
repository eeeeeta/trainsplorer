//! Utilities for applying database migrations.
use crate::traits::*;
use crate::errors::{Result, SqlError};
use chrono::{NaiveDateTime, Utc};
use log::*;

pub struct MigrationEntry {
    pub id: i32,
    pub timestamp: NaiveDateTime
}
impl DbType for MigrationEntry {
    fn table_name() -> &'static str {
        "migration_entries"
    }
    fn from_row(row: &Row, s: usize) -> RowResult<Self> {
        Ok(Self {
            id: row.get(s + 0)?,
            timestamp: row.get(s + 1)?
        })
    }
}
impl InsertableDbType for MigrationEntry {
    type Id = ();
    fn insert_self(&self, conn: &Connection) -> RowResult<()> {
        let mut stmt = conn.prepare("INSERT INTO migration_entries
                                     (id, timestamp) VALUES (?, ?)")?;
        stmt.insert(params![self.id, self.timestamp])?;
        Ok(())
    }
}
pub struct Migration {
    pub id: i32,
    pub name: &'static str,
    pub up: &'static str,
}
impl Migration {
    pub fn up(&self, conn: &mut Connection) -> Result<()> {
        conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
        let trans = conn.transaction()?;
        info!("executing up stage for migration {}: {}", self.id, self.name);
        if MigrationEntry::from_select(&trans, "WHERE id >= $1", &[&self.id])?.len() > 0 {
            error!("Attempted to apply migration out of order!");
            return Err(SqlError::MigrationOutOfOrder(self.id));
        }
        trans.execute_batch(self.up)?;
        let ent = MigrationEntry {
            id: self.id,
            timestamp: Utc::now().naive_utc()
        };
        ent.insert_self(&trans)?;
        trans.commit()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(())
    }
}
#[macro_export]
macro_rules! migration {
    ($id:expr, $name:expr) => {
        Migration {
            id: $id,
            name: $name,
            up: include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/", $id, "_", $name, "_up.sql")),
        }
    }
}
pub fn get_last_migration(conn: &Connection) -> Result<Option<i32>> {
    Ok(MigrationEntry::from_select(conn, "ORDER BY id DESC LIMIT 1", &[])?
       .into_iter()
       .nth(0)
       .map(|x| x.id))
}
pub fn initialize_migrations(conn: &Connection) -> Result<()> {
    debug!("initializing migrations...");
    conn.execute_batch(r#"CREATE TABLE IF NOT EXISTS migration_entries (
        id INT UNIQUE NOT NULL,
        timestamp TIMESTAMP NOT NULL
    )"#)?;
    Ok(())
}
pub fn run_pending_migrations(conn: &mut Connection, migrations: &[Migration]) -> Result<()> {
    let mut last_migration = get_last_migration(conn)?
        .unwrap_or(::std::i32::MIN);
    debug!("last migration ID = {}", last_migration);
    if last_migration > migrations.last().unwrap().id {
        error!("Database too new! Please upgrade trainsplorer.");
        return Err(SqlError::DatabaseTooNew);
    }
    for mig in migrations.iter() {
        if mig.id > last_migration {
            mig.up(conn)?;
            last_migration = mig.id;
        }
    }
    debug!("migrations complete");
    Ok(())
}
