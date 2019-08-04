//! Basic error handling.

pub use failure::Error;

pub type Result<T, E = Error> = ::std::result::Result<T, E>;
