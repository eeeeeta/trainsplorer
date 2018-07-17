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
    #[fail(display = "Darwin provided a forecast, but no location timings")]
    DarwinTimingsMissing,
    #[fail(display = "Train #{} already activated with TRUST id {} (parent sched #{} and date {}), but {} also matches", id, orig_trust_id, parent_sched, date, new_trust_id)]
    DoubleActivation {
        id: i32,
        orig_trust_id: String,
        parent_sched: i32,
        date: NaiveDate,
        new_trust_id: String
    },
    #[fail(display = "Couldn't find any trains for RID {} (UID {}, start_date {})", rid, uid, start_date)]
    RidLinkFailed {
        rid: String,
        uid: String,
        start_date: NaiveDate
    },
    #[fail(display = "More than one train for RID {} (UID {}, start_date {})", rid, uid, start_date)]
    AmbiguousTrains {
        rid: String,
        uid: String,
        start_date: NaiveDate
    },
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
    #[fail(display = "Schedules #{} and #{} are both authoritative for Darwin activation", _0, _1)]
    TwoAuthoritativeSchedulesDarwin(i32, i32),
    #[fail(display = "No schedules are authoritative (UID {}, start {}, stp_indicator {:?}, src {})", _0, _1, _2, _3)]
    NoAuthoritativeSchedules(String, NaiveDate, StpIndicator, i32),
    #[fail(display = "No schedules are authoritative for Darwin RID {} (uid {}, on date {})", rid, uid, start_date)]
    NoAuthoritativeSchedulesDarwin {
        rid: String,
        uid: String,
        start_date: NaiveDate
    },
    #[fail(display = "No train found for ID {} on date {}", _0, _1)]
    NoTrainFound(String, NaiveDate),
    #[fail(display = "Failed to find any schedule movements (sched #{}, actions {:?}, tiplocs {:?}, time {:?})", _0, _1, _2, _3)]
    NoMovementsFound(i32, Vec<i32>, Vec<String>, Option<NaiveTime>),
    #[fail(display = "Multiple errors: {:?}", _0)]
    MultipleFailures(Vec<NrodError>)
}

impl_from_for_error! {
    NrodError,
    IoError => Io,
    PgError => Pg,
    SerdeError => Serde,
    OsmsError => Db
}
