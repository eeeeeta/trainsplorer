use std::io::Error as IoError;
use postgres::error::Error as PgError;
use serde_json::Error as SerdeError;
#[derive(Debug, Fail)]
pub enum OsmsError {
    #[fail(display = "I/O error: {}", _0)]
    Io(#[cause] IoError),
    #[fail(display = "PostgreSQL error: {}", _0)]
    Pg(#[cause] PgError),
    #[fail(display = "Serde error: {}", _0)]
    Serde(#[cause] SerdeError),
    #[fail(display = "Database inconsistency: {}", _0)]
    DatabaseInconsistency(&'static str),
    #[fail(display = "Station #{} couldn't be found", _0)]
    StationNotFound(i32),
    #[fail(display = "Crossing {} couldn't be found", _0)]
    CrossingNotFound(i32),
    #[fail(display = "Schedule file is invalid")]
    InvalidScheduleFile,
    #[fail(display = "Schedule file has already been inserted")]
    ScheduleFileExists,
    #[fail(display = "Bad schedule file import request: {}", _0)]
    ScheduleFileImportInvalid(&'static str),
    #[fail(display = "Station #{} isn't in the same graph part as station #{}.", to, from)]
    IncorrectGraphPart {
        from: i32,
        to: i32
    },
    #[fail(display = "No authoritative schedules.")]
    NoAuthoritativeSchedules,
    #[fail(display = "Something the developers thought was impossible happened.")]
    ExtraterrestrialActivity
}
pub type Result<T> = ::std::result::Result<T, OsmsError>;

impl_from_for_error! {
    OsmsError,
    IoError => Io,
    PgError => Pg,
    SerdeError => Serde
}
