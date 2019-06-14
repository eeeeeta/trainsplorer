//! Error handling.

use tspl_util::impl_from_for_error;
use failure_derive::Fail;
use rmp_serde::decode::Error as RmpDecodeError;
use rmp_serde::encode::Error as RmpEncodeError;
use std::io::Error as IoError;
use nng::Error as NngError;

pub type Result<T> = ::std::result::Result<T, ProtoError>;

#[derive(Fail, Debug)]
pub enum ProtoError {
    #[fail(display = "Incorrect protocol API version {} in message", _0)]
    IncorrectProtoVersion(u8),
    #[fail(display = "Incorrect app API version: message is {}, we're {}", message, cur)]
    IncorrectAppVersion {
        message: u8,
        cur: u8
    },
    #[fail(display = "Malformed response type: {}", _0)]
    MalformedResponse(&'static str),
    #[fail(display = "Failed to deserialize: {}", _0)]
    Deserialize(RmpDecodeError),
    #[fail(display = "Failed to serialize: {}", _0)]
    Serialize(RmpEncodeError),
    #[fail(display = "I/O error: {}", _0)]
    Io(IoError),
    #[fail(display = "nng error: {}", _0)]
    Nng(NngError)
}
impl_from_for_error!(ProtoError,
                     RmpDecodeError => Deserialize,
                     RmpEncodeError => Serialize,
                     IoError => Io,
                     NngError => Nng);
