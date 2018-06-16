use db::*;
use chrono::{NaiveDateTime, Utc};
use errors::*;

pub struct MigrationEntry {
    pub id: i32,
    pub timestamp: NaiveDateTime
}
impl DbType for MigrationEntry {
    fn table_name() -> &'static str {
        "migration_entries"
    }
    fn from_row(row: &Row) -> Self {
        Self {
            id: row.get(0),
            timestamp: row.get(1)
        }
    }
}
impl InsertableDbType for MigrationEntry {
    type Id = ();
    fn insert_self<T: GenericConnection>(&self, conn: &T) -> Result<()> {
        conn.execute("INSERT INTO migration_entries
                      (id, timestamp) VALUES ($1, $2)",
                      &[&self.id, &self.timestamp])?;
        Ok(())
    }
}
pub struct Migration {
    pub id: i32,
    pub name: &'static str,
    pub up: &'static str,
    pub down: &'static str
}
impl Migration {
    pub fn up<T: GenericConnection>(&self, conn: &T) -> Result<()> {
        let trans = conn.transaction()?;
        debug!("executing up stage for migration {}: {}", self.id, self.name);
        if MigrationEntry::from_select(&trans, "WHERE id >= $1", &[&self.id])?.len() > 0 {
            error!("Attempted to apply migration out of order!");
            return Err(OsmsError::MigrationOutOfOrder(self.id));
        }
        trans.batch_execute(self.up)?;
        let ent = MigrationEntry {
            id: self.id,
            timestamp: Utc::now().naive_utc()
        };
        ent.insert_self(&trans)?;
        trans.commit()?;
        Ok(())
    }
    pub fn down<T: GenericConnection>(&self, conn: &T) -> Result<()> {
        let trans = conn.transaction()?;
        debug!("executing down stage for migration {}: {}", self.id, self.name);
        if MigrationEntry::from_select(&trans, "WHERE id > $1", &[&self.id])?.len() > 0 {
            error!("Attempted to undo migration out of order!");
            return Err(OsmsError::MigrationOutOfOrder(self.id));
        }
        if MigrationEntry::from_select(&trans, "WHERE id = $1", &[&self.id])?.len() == 0 {
            error!("Attempted to undo migration that hasn't been applied!");
            return Err(OsmsError::MigrationNotApplied(self.id));
        }
        trans.batch_execute(self.down)?;
        trans.execute("DELETE FROM migration_entries WHERE id = $1", &[&self.id])?;
        trans.commit()?;
        Ok(())
    }
}
macro_rules! migration {
    ($id:expr, $name:expr) => {
        Migration {
            id: $id,
            name: $name,
            up: include_str!(concat!("../migrations/", $id, "_", $name, "_up.sql")),
            down: include_str!(concat!("../migrations/", $id, "_", $name, "_down.sql")),
        }
    }
}
pub static MIGRATIONS: [Migration; 1] = [
    migration!(0, "initial")
];
pub fn get_last_migration<T: GenericConnection>(conn: &T) -> Result<Option<i32>> {
    Ok(MigrationEntry::from_select(conn, "ORDER BY id DESC LIMIT 1", &[])?
       .into_iter()
       .nth(0)
       .map(|x| x.id))
}
pub fn initialize_migrations<T: GenericConnection>(conn: &T) -> Result<()> {
    debug!("initializing migrations...");
    conn.execute(r#"CREATE TABLE IF NOT EXISTS migration_entries (
        id INT UNIQUE NOT NULL,
        timestamp TIMESTAMP NOT NULL
    )"#, &[])?;
    let mut last_migration = get_last_migration(conn)?
        .unwrap_or(::std::i32::MIN);
    debug!("last migration ID = {}", last_migration);
    if last_migration > MIGRATIONS.last().unwrap().id {
        error!("Database too new! Please upgrade osm-signal.");
        return Err(OsmsError::DatabaseTooNew);
    }
    for mig in MIGRATIONS.iter() {
        if mig.id > last_migration {
            mig.up(conn)?;
            last_migration = mig.id;
        }
    }
    debug!("migrations complete");
    Ok(())
}
pub fn undo_migrations_to<T: GenericConnection>(conn: &T, id: i32) -> Result<()> {
    debug!("rolling back migrations greater than id {}", id);
    let migrations = MigrationEntry::from_select(conn, "WHERE id > $1 ORDER BY id DESC", &[])?;
    for ent in migrations {
        if let Ok(elem) = MIGRATIONS.binary_search_by_key(&ent.id, |m| m.id) {
            let mig = &MIGRATIONS[elem];
            mig.down(conn)?;
        }
    }
    debug!("done!");
    Ok(())
}
