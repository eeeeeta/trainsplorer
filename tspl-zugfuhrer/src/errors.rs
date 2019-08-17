//! Basic error handling.

pub use failure::Error;
use failure_derive::Fail;
use tspl_util::impl_from_for_error;
use tspl_util::http::StatusCode;
use tspl_sqlite::errors::{SqlError, PoolError};
use tspl_sqlite::rusqlite::Error as RsqlError;
use reqwest::Error as ReqwestError;
use tspl_util::rpc::RpcError;

/// Error that could occur when processing a request.
#[derive(Fail, Debug)]
pub enum ZugError {
    /// The given entity was not found.
    #[fail(display = "not found")]
    NotFound,
    /// Failed to find movements matching that description.
    #[fail(display = "movements not found")]
    MovementsNotFound,
    /// The API path doesn't exist.
    #[fail(display = "invalid path")]
    InvalidPath,
    /// More than one movement matched the information provided.
    #[fail(display = "movements ambiguous")]
    MovementsAmbiguous,
    /// Headers missing or failed to parse.
    #[fail(display = "bad request")]
    HeadersMissing,
    /// RPC error.
    #[fail(display = "RPC: {}", _0)]
    Rpc(RpcError),
    /// SQL error from tspl-sqlite.
    #[fail(display = "tspl-sqlite: {}", _0)]
    Sql(SqlError),
    /// SQL error from rusqlite.
    #[fail(display = "rusqlite: {} ({})", _1, _0)]
    Rsql(String, RsqlError),
    /// r2d2 database error.
    #[fail(display = "r2d2: {}", _0)]
    Pool(PoolError),
    /// reqwest error.
    #[fail(display = "reqwest: {}", _0)]
    Reqwest(ReqwestError)
}

impl StatusCode for ZugError {
    fn status_code(&self) -> u16 {
        use self::ZugError::*;

        match *self {
            NotFound => 404,
            MovementsNotFound => 404,
            MovementsAmbiguous => 409,
            HeadersMissing => 400,
            InvalidPath => 400,
            Rpc(ref r) => r.status_code(),
            Pool(_) => 503,
            _ => 500
        }
    }
}

pub trait OptionalExt<T> {
    fn optional(self) -> ZugResult<Option<T>>;
}
impl<T, E> OptionalExt<T> for Result<T, E> where E: Into<ZugError> {
    fn optional(self) -> ZugResult<Option<T>> {
        match self.map_err(|e| e.into()) {
            Ok(x) => Ok(Some(x)),
            Err(ZugError::NotFound) => Ok(None),
            Err(ZugError::Rpc(RpcError::RemoteNotFound)) => Ok(None),
            Err(e) => Err(e)
        }
    }
}

impl From<RsqlError> for ZugError {
    fn from(r: RsqlError) -> ZugError {
        ZugError::Rsql(format!("{:?}", r), r)
    }
}
impl_from_for_error!(ZugError,
                     ReqwestError => Reqwest,
                     SqlError => Sql,
                     PoolError => Pool,
                     RpcError => Rpc);

pub type ZugResult<T> = ::std::result::Result<T, ZugError>;
pub type Result<T, E = Error> = ::std::result::Result<T, E>;
