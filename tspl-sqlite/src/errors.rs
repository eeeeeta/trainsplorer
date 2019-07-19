//! Error handling.

use tspl_util::impl_from_for_error;
use failure_derive::Fail;
use rusqlite::Error as SqliteError;
pub use r2d2::Error as PoolError;

pub type Result<T> = ::std::result::Result<T, SqlError>;

#[derive(Fail, Debug)]
pub enum SqlError {
    #[fail(display = "SQLite error: {}", _0)]
    Sqlite(SqliteError),
    #[fail(display = "Migration {} applied out of order.", _0)]
    MigrationOutOfOrder(i32),
    #[fail(display = "Database too new!")]
    DatabaseTooNew
}
impl_from_for_error!(SqlError,
                     SqliteError => Sqlite);
