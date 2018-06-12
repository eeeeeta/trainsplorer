use std::io::Error as IoError;
use r2d2_postgres::postgres::error::Error as PgError;
use serde_json::Error as SerdeError;
use osms_db::errors::OsmsError;
use serde_json::Value;
use ntrod_types::cif::StpIndicator;
use chrono::{NaiveDate, NaiveTime};

#[derive(Debug, Fail)]
pub enum NrodError {
    #[fail(display = "I/O error: {}", _0)]
    Io(#[cause] IoError),
    #[fail(display = "PostgreSQL error: {}", _0)]
    Pg(#[cause] PgError),
    #[fail(display = "Serde error: {}", _0)]
    Serde(#[cause] SerdeError),
    #[fail(display = "osms-db error: {}", _0)]
    Db(#[cause] OsmsError),
    #[fail(display = "VSTP schedule was missing a schedule segment")]
    NoScheduleSegment,
    #[fail(display = "ntrod-types failed to parse movement body: {:?}", _0)]
    UnknownMvtBody(Value),
    #[fail(display = "Message type {} is unimplemented", _0)]
    UnimplementedMessageType(String),
    #[fail(display = "Failed to find a schedule (UID {}, start {}, stp_indicator {:?}, src {}) when processing activation for {} on {}", train_uid, start_date, stp_indicator, source, train_id, date)]
    NoSchedules {
        train_uid: String,
        start_date: NaiveDate, 
        stp_indicator: StpIndicator,
        source: i32,
        train_id: String,
        date: NaiveDate
    },
    #[fail(display = "Schedules #{} and #{} are both authoritative!", _0, _1)]
    TwoAuthoritativeSchedules(i32, i32),
    #[fail(display = "No schedules are authoritative (UID {}, start {}, stp_indicator {:?}, src {})", _0, _1, _2, _3)]
    NoAuthoritativeSchedules(String, NaiveDate, StpIndicator, i32),
    #[fail(display = "No train found for ID {} on date {}", _0, _1)]
    NoTrainFound(String, NaiveDate),
    #[fail(display = "Failed to find any schedule movements (sched #{}, actions {:?}, tiplocs {:?}, time {:?}", _0, _1, _2, _3)]
    NoMovementsFound(i32, Vec<i32>, Vec<String>, Option<NaiveTime>)
}

impl_from_for_error! {
    NrodError,
    IoError => Io,
    PgError => Pg,
    SerdeError => Serde,
    OsmsError => Db
}
