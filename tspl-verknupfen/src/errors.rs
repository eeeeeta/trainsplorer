//! Standard fare error handling.
//!
//! (Coming up with witty headings for these standard files is becoming tiresome.)

pub use failure::Error;
use failure_derive::Fail;
use tspl_util::impl_from_for_error;
use tspl_util::http::StatusCode;
use tspl_util::rpc::RpcError;

/// Error that could occur when processing a request.
#[derive(Fail, Debug)]
pub enum VerknupfenError {
    /// The API path doesn't exist.
    #[fail(display = "invalid path")]
    InvalidPath,
    /// Invalid response from remote microservice.
    #[fail(display = "remote invariants violated")]
    RemoteInvariantsViolated,
    /// RPC error.
    #[fail(display = "RPC: {}", _0)]
    Rpc(RpcError),
}

impl StatusCode for VerknupfenError {
    fn status_code(&self) -> u16 {
        use self::VerknupfenError::*;

        match *self {
            InvalidPath => 400,
            Rpc(ref r) => r.status_code(),
            RemoteInvariantsViolated => 502,
            _ => 500
        }
    }
}

pub type VerknupfenResult<T, E = VerknupfenError> = ::std::result::Result<T, E>;
pub type Result<T, E = Error> = ::std::result::Result<T, E>;

impl_from_for_error!(VerknupfenError,
                     RpcError => Rpc);
