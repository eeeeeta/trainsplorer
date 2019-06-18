//! Microservice protocol - i.e. what this thing actually exposes to the world.

use tspl_sqlite::uuid::Uuid;
use serde_derive::{Serialize, Deserialize};
use failure_derive::{Fail};
use chrono::NaiveDate;
use crate::types::{Schedule, ScheduleMvt};
use crate::download::JobType;
use tspl_proto::RpcInterface;
use tspl_proto::wire::RpcResponse;

pub type FahrplanResponse<'a> = RpcResponse<'a, FahrplanRpc>;

/// Error that could occur when processing a request.
#[derive(Serialize, Deserialize, Fail, Debug)]
pub enum FahrplanError {
    /// The given entity was not found.
    #[fail(display = "fahrplan entity not found")]
    NotFound,
    /// Some internal error occurred while processing the request.
    #[fail(display = "Internal service error: {}", _0)]
    InternalError(String)
}

/// A request issued to `tspl-fahrplan` by another microservice.
#[derive(Serialize, Deserialize, Debug)]
pub enum FahrplanRequest {
    /// Find all schedules with a given `uid`.
    ///
    /// ## Returns
    ///
    /// `Vec<Schedule>`
    FindSchedulesWithUid {
        uid: String
    },
    /// Find the authoritative schedule for a given date and UID.
    ///
    /// ## Usecases
    ///
    /// - This function is pretty useful for Darwin activations (where
    ///   this is all you get in order to uniquely identify a schedule).
    ///
    /// ## Returns
    ///
    /// `Schedule` (or `NotFound`)
    FindScheduleOnDate {
        /// Schedule UID (e.g. 'W91071') to filter on.
        uid: String,
        /// The date on which the returned schedule must be authoritative.
        on_date: NaiveDate,
        /// Schedule source (i.e. ITPS/VSTP). See `Schedule`'s associated
        /// consts.
        ///
        /// It's likely that you'll be setting this to ITPS, given VSTP
        /// is only really used in TRUST activations.
        source: u8
    },
    /// Find the appropriate authoritative schedule for a TRUST activation.
    ///
    /// ## Usecases
    ///
    /// - TRUST activation messages literally have these fields.
    /// - This also lets you retrieve a schedule via its primary key (these
    ///   four fields uniquely identify a schedule version).
    ///
    /// ## Returns
    ///
    /// `Schedule` (or `NotFound`)
    FindScheduleForActivation {
        /// Schedule UID (e.g. 'W91071').
        uid: String,
        /// STP indicator in single-char format (e.g. 'O').
        stp_indicator: String,
        /// Schedule start date.
        start_date: NaiveDate,
        /// Schedule source (i.e. ITPS/VSTP).
        source: u8
    },
    /// Given a schedule's trainsplorer UUID, return details about it and
    /// all of its movements.
    ///
    /// ## Returns
    ///
    /// `Option<ScheduleDetails>`
    RequestScheduleDetails(Uuid),
    /// Queue the given database update job.
    ///
    /// ## Returns
    ///
    /// `()`
    QueueUpdateJob(JobType)
}

/// The complete details about a schedule stored in the database.
#[derive(Serialize, Deserialize)]
pub struct ScheduleDetails {
    /// Actual schedule object.
    pub sched: Schedule,
    /// Schedule movements, in the proper order.
    pub mvts: Vec<ScheduleMvt>
}
/// tspl-fahrplan RPC interface.
pub struct FahrplanRpc;

impl RpcInterface for FahrplanRpc {
    type Request = FahrplanRequest;
    type Error = FahrplanError;

    fn api_version() -> u8 {
        0
    }
}