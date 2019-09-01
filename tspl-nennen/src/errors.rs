//! Error handling (this one's boring because it's not a microservice)

pub use failure::Error;
pub type Result<T, E = Error> = ::std::result::Result<T, E>;
