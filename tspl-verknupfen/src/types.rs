//! API types.

use chrono::prelude::*;
use serde_derive::{Serialize, Deserialize};
use std::collections::HashMap;
use tspl_zugfuhrer::types::Train;
use tspl_fahrplan::types::Schedule;

/// A time, together with source information.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct TimeWithSource {
    /// The source of this time. Mirrors the value of
    /// `TrainMvt::source`, from tspl-zugfuhrer, and uses those associated
    /// constants.
    pub source: i32,
    /// The actual time.
    pub time: NaiveTime
}

/// Source information for a deduplicated movement.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DeduplicatedMvtSource {
    Schedule(i64),
    Train(i64)
}
/// A deduplicated movement, providing aggregated information
/// about a train's movement through a given location.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DeduplicatedMvt {
    /// Where this movement is from, i.e. the ID of its parent schedule or train.
    pub src: DeduplicatedMvtSource,
    /// Timing Point Location where this movement happens.
    pub tiploc: String,
    /// What action actually happens here (see `TrainMvt::action`)
    pub action: u8,
    /// The time of the movement, either expected or actual.
    pub time: TimeWithSource,
    /// Whether this movement has actually happened and now describes
    /// a past event, or whether the time is still a prediction.
    pub actual: bool,
    /// If applicable, the time at which the train was originally
    /// scheduled.
    pub time_scheduled: Option<TimeWithSource>,
    /// Is this movement cancelled?
    pub canx: bool,
    /// The scheduled platform (i.e. the one from TRUST)
    pub pfm_scheduled: Option<String>,
    /// The current or expected platform for this movement.
    pub pfm_actual: Option<String>,
    /// Should platform information be shown to users?
    pub pfm_suppr: bool
}

/// The result of a call to get_mvts_passing_through().
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MvtQueryResponse {
    pub mvts: Vec<DeduplicatedMvt>,
    pub schedules: HashMap<i64, Schedule>,
    pub trains: HashMap<i64, Train>
}
