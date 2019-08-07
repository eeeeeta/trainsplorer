//! Basic error handling.

pub use failure::Error;
use failure_derive::Fail;
use tspl_util::impl_from_for_error;
use tspl_sqlite::errors::{SqlError, PoolError};
use tspl_sqlite::rusqlite::Error as RsqlError;
use reqwest::Error as ReqwestError;

/// Error that could occur when processing a request.
#[derive(Fail, Debug)]
pub enum ZugError {
    /// The given entity was not found.
    #[fail(display = "not found")]
    NotFound,
    /// Error from tspl-fahrplan.
    #[fail(display = "fahrplan error (code {}): {}", _0, _1)]
    Fahrplan(u16, String),
    /// The remote entity was not found.
    #[fail(display = "not found (remote)")]
    RemoteNotFound,
    /// The remote service was unavailable.
    #[fail(display = "remote service unavailable")]
    RemoteServiceUnavailable,
    /// More than one movement matched the information provided.
    #[fail(display = "movements ambiguous")]
    MovementsAmbiguous,
    /// SQL error from tspl-sqlite.
    #[fail(display = "tspl-sqlite: {}", _0)]
    Sql(SqlError),
    /// SQL error from rusqlite.
    #[fail(display = "rusqlite: {}", _0)]
    Rsql(RsqlError),
    /// r2d2 database error.
    #[fail(display = "r2d2: {}", _0)]
    Pool(PoolError),
    /// reqwest error.
    #[fail(display = "reqwest: {}", _0)]
    Reqwest(ReqwestError)
}

impl ZugError {
    pub fn status_code(&self) -> u16 {
        use self::ZugError::*;

        match *self {
            NotFound => 404,
            Fahrplan(..) => 502,
            RemoteNotFound => 404,
            MovementsAmbiguous => 409,
            Pool(_) => 503,
            RemoteServiceUnavailable => 503,
            _ => 500
        }
    }
}

pub trait OptionalExt<T> {
    fn optional(self) -> ZugResult<Option<T>>;
}
impl<T> OptionalExt<T> for ZugResult<T> {
    fn optional(self) -> ZugResult<Option<T>> {
        match self {
            Ok(x) => Ok(Some(x)),
            Err(ZugError::NotFound) => Ok(None),
            Err(ZugError::RemoteNotFound) => Ok(None),
            Err(e) => Err(e)
        }
    }
}

impl_from_for_error!(ZugError,
                     ReqwestError => Reqwest,
                     RsqlError => Rsql,
                     SqlError => Sql,
                     PoolError => Pool);

pub type ZugResult<T> = ::std::result::Result<T, ZugError>;
pub type Result<T, E = Error> = ::std::result::Result<T, E>;
