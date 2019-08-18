//! Error handling (you know the drill)

pub use failure::Error;
pub type Result<T, E = Error> = ::std::result::Result<T, E>;
