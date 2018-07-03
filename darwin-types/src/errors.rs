//! Error handling.
pub type Result<T> = ::std::result::Result<T, DarwinError>;
use std::str::ParseBoolError;
use xml::reader::Error as Xml;
use xml::name::OwnedName;
use xml::reader::XmlEvent;
use std::num::ParseIntError as Int;
use chrono::format::ParseError as Chrono;

#[derive(Debug, Fail)]
pub enum DarwinError {
    #[fail(display = "error parsing date/time: {}", _0)]
    Chrono(Chrono),
    #[fail(display = "failed to parse integer: {}", _0)]
    ParseInt(Int),
    #[fail(display = "got end for unexpected element {}", _0)]
    EndMismatch(OwnedName),
    #[fail(display = "unexpected event {:?}", _0)]
    UnexpectedEvent(XmlEvent),
    #[fail(display = "got unexpected start of element {}", _0)]
    UnexpectedStart(OwnedName),
    #[fail(display = "failed to parse bool: {}", _0)]
    ParseBoolError(#[cause] ParseBoolError),
    #[fail(display = "expected {}, got {}", _0, _1)]
    Expected(&'static str, String),
    #[fail(display = "missing {}", _0)]
    Missing(&'static str),
    #[fail(display = "XML parse error: {}", _0)]
    XmlError(#[cause] Xml),
    #[fail(display = "Unexpected end of input")]
    UnexpectedEnd
}
impl_from_for_error! {
    DarwinError,
    ParseBoolError => ParseBoolError,
    Xml => XmlError,
    Chrono => Chrono,
    Int => ParseInt
}
