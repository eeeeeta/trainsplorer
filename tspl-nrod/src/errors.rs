//! Bog-standard error handling.

use failure::Error;

pub type Result<T, E = Error> = ::std::result::Result<T, E>;
