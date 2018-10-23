use std::io::Error as IoError;
use r2d2_postgres::postgres::error::Error as PgError;
use serde_json::Error as SerdeError;
use osms_db::errors::OsmsError;
use serde_json::Value;
use ntrod_types::cif::StpIndicator;
use chrono::{NaiveDate, NaiveTime};
use super::NtrodWorker;

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
    #[fail(display = "Duplicate VSTP schedule (UID {}, start {}, stp_indicator {:?}, src {})", train_uid, start_date, stp_indicator, source)]
    DuplicateVstpSchedule {
        train_uid: String,
        start_date: NaiveDate, 
        stp_indicator: StpIndicator,
        source: i32,
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
    #[fail(display = "Failed to find any schedule movements with Darwin schedule (sched #{}, actions {:?}, tiplocs {:?}, time {:?})", _0, _1, _2, _3)]
    NoMovementsFoundDarwin(i32, Vec<i32>, Vec<String>, Option<NaiveTime>),
    #[fail(display = "Multiple errors: {:?}", _0)]
    MultipleFailures(Vec<NrodError>)
}

impl NrodError {
    pub fn send_to_stats(&self, prefix: &'static str, worker: &mut NtrodWorker) {
        match *self {
            NrodError::MultipleFailures(ref fails) => {
                for fail in fails {
                    fail.send_to_stats(prefix, worker);
                }
            },
            ref x => {
                worker.incr(&format!("{}.{}", prefix, x.stats_name()));
            }
        }
    }
    pub fn stats_name(&self) -> &'static str {
        use self::NrodError::*;

        match *self {
            Io(..) => "io",
            Pg(..) => "pg",
            Serde(..) => "serde",
            Db(..) => "db",
            NoScheduleSegment => "no_schedule_segment",
            DuplicateVstpSchedule { .. } => "duplicate_vstp_schedule",
            UnknownMvtBody(..) => "unknown_mvt_body",
            UnimplementedMessageType(..) => "unimplemented_message_type",
            DarwinTimingsMissing => "darwin_timings_missing",
            NoSchedules { .. } => "no_schedules",
            TwoAuthoritativeSchedules(..) => "two_authoritative_schedules",
            TwoAuthoritativeSchedulesDarwin(..) => "two_authoritative_schedules_darwin",
            NoAuthoritativeSchedules(..) => "no_authoritative_schedules",
            NoAuthoritativeSchedulesDarwin { .. } => "no_authoritative_schedules_darwin",
            NoTrainFound { .. } => "no_train_found",
            NoMovementsFound { .. } => "no_movements_found",
            NoMovementsFoundDarwin { .. } => "no_movements_found_darwin",
            MultipleFailures { .. } => "multiple_failures"
        }
    }
}

impl_from_for_error! {
    NrodError,
    IoError => Io,
    PgError => Pg,
    SerdeError => Serde,
    OsmsError => Db
}
