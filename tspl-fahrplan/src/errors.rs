//! Error handling.

use failure_derive::Fail;
use tspl_util::impl_from_for_error;
use tspl_sqlite::errors::{SqlError, PoolError};
use tspl_util::http::StatusCode;

/// Error that could occur when processing a request.
#[derive(Fail, Debug)]
pub enum FahrplanError {
    /// The given entity was not found.
    #[fail(display = "not found")]
    NotFound,
    /// The API path doesn't exist.
    #[fail(display = "invalid path")]
    InvalidPath,
    #[fail(display = "STP indicators equal")]
    StpIndicatorsEqual,
    /// SQL error from tspl-sqlite.
    #[fail(display = "tspl-sqlite: {}", _0)]
    Sql(SqlError),
    /// r2d2 database error.
    #[fail(display = "r2d2: {}", _0)]
    Pool(PoolError),
    /// failure to send something across an mpsc channel
    #[fail(display = "failed to queue background job")]
    JobQueueError
}

impl StatusCode for FahrplanError {
    fn status_code(&self) -> u16 {
        use self::FahrplanError::*;

        match *self {
            NotFound => 404,
            InvalidPath => 400,
            _ => 500
        }
    }
}

pub type FahrplanResult<T> = ::std::result::Result<T, FahrplanError>;
pub type Result<T> = ::std::result::Result<T, failure::Error>;

impl_from_for_error!(FahrplanError,
                     SqlError => Sql,
                     PoolError => Pool);
