//! Error handling.

use failure_derive::Fail;
use tspl_util::impl_from_for_error;
use google_storage1::Error as GoogError;
use std::io::Error as IoError;
use hyper::status::StatusCode;

/// Cloud storage error.
#[derive(Fail, Debug)]
pub enum GcsError {
    /// The given entity was not found.
    #[fail(display = "not found")]
    NotFound,
    /// A non-success status code was received.
    #[fail(display = "request returned {} ({:?})", _0, _1)]
    FailedStatus(&'static str, StatusCode),
    /// I/O error.
    #[fail(display = "I/O: {}", _0)]
    Io(IoError),
    /// Any other error from google_storage1.
    #[fail(display = "google_storage1: {}", _0)]
    Goog(String)
}

impl From<GoogError> for GcsError {
    fn from(g: GoogError) -> GcsError {
        match g {
            GoogError::Failure(resp) => {
                match resp.status {
                    StatusCode::NotFound => GcsError::NotFound,
                    oth => {
                        let reason = oth.canonical_reason().unwrap_or("unknown");
                        GcsError::FailedStatus(reason, oth)
                    }
                }
            },
            oth => GcsError::Goog(format!("{:?}", oth))
        }
    }
}

pub type GcsResult<T> = ::std::result::Result<T, GcsError>;

impl_from_for_error!(GcsError,
                     IoError => Io);
