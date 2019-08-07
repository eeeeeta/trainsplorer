//! Quasi-ORM traits to make working with SQL easier.
//!
//! Also re-exports commonly-used `rusqlite` types and macros.

pub use rusqlite::{Connection, Row};
pub use rusqlite::Result as RowResult;
pub use rusqlite::{params, NO_PARAMS};
pub use uuid::Uuid;
use crate::errors::Result;
use rusqlite::types::ToSql;

/// A Rust type representing a row in some SQL table.
pub trait DbType: Sized {
    /// The name of the table represented by this type.
    fn table_name() -> &'static str;
    /// Converts an untyped SQLite Row into this type.
    fn from_row(row: &Row) -> RowResult<Self>;
    /// Convenience function for running a SELECT for this type, based on a given predicate
    /// `where_clause` and its `args`.
    fn from_select(conn: &Connection, where_clause: &str, args: &[&dyn ToSql]) -> Result<Vec<Self>> {
        let query = format!("SELECT * FROM {} {}", Self::table_name(), where_clause);
        let mut stmt = conn.prepare(&query)?;
        let rows = stmt.query_map(args, |row| Self::from_row(row))?;
        let mut ret = vec![];
        for row in rows {
            ret.push(row?);
        }
        Ok(ret)
    }
}
/// A `DbType` that can be inserted as well as queried.
pub trait InsertableDbType {
    /// The primary key type.
    type Id;
    /// Insert this type into the database, returning its new primary key (if any).
    fn insert_self(&self, conn: &Connection) -> RowResult<Self::Id>;
}
