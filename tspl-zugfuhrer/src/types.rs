//! Database types for live train info.

use tspl_sqlite::traits::*;
use chrono::*;

/// A live train object, representing a live or historic running of a train.
///
/// ## Identification
///
/// Like schedules, trains have their own "trainsplorer ID", which is used
/// as an opaque identifier for other services to report updates to the train's
/// running.
///
/// ## Relationship to schedules
///
/// Trains are created from a parent schedule, and retain information about
/// which schedule this was (primarily to enable nice UI features like 'view
/// other trains running to this schedule').
///
/// However, this schedule can be from multiple different sources - VSTP,
/// Darwin, or CIF/ITPS. The train movements for the train incorporate all
/// available schedule data for the train from these sources, often copied
/// from the relevant schedule(s).
///
/// ## Deduplication
///
/// Keeping information about the originating schedule also allows the display
/// of the schedule to be suppressed later, when something attempts to combine
/// train running information with schedule data.
pub struct Train {
    /// Internal primary key.
    pub id: i64,
    /// Train's trainsplorer ID.
    pub tspl_id: Uuid,
    /// Parent schedule UID.
    pub parent_uid: String,
    /// Parent schedule start date.
    pub parent_start_date: NaiveDate,
    /// Parent schedule STP indicator.
    pub parent_stp_indicator: String,
    /// Date this train was running on.
    pub date: NaiveDate,
    /// TRUST ID for this train.
    pub trust_id: Option<String>,
    /// Darwin RID for this train.
    pub darwin_rid: Option<String>,
    /// Headcode / signalling ID, if there is one.
    pub headcode: Option<String>,
    /// Whether or not this train's schedule crosses over into the next day.
    pub crosses_midnight: bool,
    /// The source used to retrieve the parent schedule at activation time.
    ///
    /// This is required to deal with fun edge cases, like VSTP schedules
    /// conflicting with ITPS schedules and ruining everything.
    pub parent_source: i32
}

impl DbType for Train {
    fn table_name() -> &'static str {
        "trains"
    }
    fn from_row(row: &Row) -> RowResult<Self> {
        Ok(Self {
            id: row.get(0),
            tspl_id: row.get(1),
            parent_uid: row.get(2),
            parent_start_date: row.get(3),
            parent_stp_indicator: row.get(4),
            date: row.get(5),
            trust_id: row.get(6),
            darwin_rid: row.get(7),
            headcode: row.get(8),
            crosses_midnight: row.get(9),
            parent_source: row.get(10)
        })
    }
}
impl InsertableDbType for Train {
    type Id = i64;
    fn insert_self(&self, conn: &Connection) -> RowResult<i64> {
        let mut stmt = conn.prepare("INSERT INTO trains
                                     (tspl_id, parent_uid, parent_start_date,
                                      parent_stp_indicator, date, trust_id,
                                      darwin_rid, headcode, crosses_midnight,
                                      parent_source)
                                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")?;
        let rid = stmt.insert(params![self.tspl_id, self.parent_uid,
                              self.parent_start_date, self.parent_stp_indicator,
                              self.date, self.trust_id, self.darwin_rid,
                              self.headcode, self.crosses_midnight, self.parent_source])?;
        Ok(rid)
    }
}
/// Describes a live update to a `ScheduleMvt`.
///
/// ## Uniqueness and deduplication
///
/// All train movements without an `updates` value are part of the train's
/// live schedule, sorted as described below.
///
/// The tuple `(parent_train, updates, tiploc, action, time, day_offset)` should
/// uniquely identify a train movement, with updates for this movement provided
/// by other movements which reference it via `updates`.
///
/// ## Sorting
///
/// Train movements, in much the same way as schedule movements,
/// should ideally be sorted by (day_offset, time, action, source),
/// for comparison to other lists of movements.
/// (indeed, this is how `Ord` is implemented)
///
/// ## Timing
///
/// **NB**: Please update the `crosses_midnight` field on the parent train
/// if adding/removing movements that cause the train to cross over past midnight.
pub struct TrainMvt {
    /// Internal primary key.
    pub id: i64,
    /// Train this is in reference to.
    pub parent_train: i64,
    /// Reference to another movement that this movement supersedes, or updates.
    ///
    /// For example, live predictions would set this `updated` field to the
    /// ITPS or Darwin train movement which they reference.
    ///
    /// In addition, this may also be used to merge Darwin and ITPS schedules - e.g.
    /// the Darwin schedule's movements could `update` the ITPS movements, in order
    /// to signify that the two movements are identical and shouldn't be counted
    /// twice.
    pub updates: Option<i64>,
    /// Timing Point Location where this movement happens.
    pub tiploc: String,
    /// What actually happens here - one of:
    ///
    /// - 0: arrival
    /// - 1: departure
    /// - 2: pass
    pub action: u8,
    /// Whether this movement actually happened (if not, this is just a prediction)
    pub actual: bool,
    /// The updated time.
    pub time: NaiveTime,
    /// The updated public (GBTT) time, if any.
    pub public_time: Option<NaiveTime>,
    /// Day offset - number of days past the schedule start date after which this
    /// movement happens.
    ///
    /// This is used for schedules crossing midnight, which will have day_offset
    /// of 1 on the movements after midnight.
    pub day_offset: u8,
    /// The source of this update - one of:
    ///
    /// - 0: `SOURCE_SCHED_ITPS` (from the CIF/ITPS schedule)
    /// - 1: `SOURCE_SCHED_DARWIN` (from the Darwin schedule)
    /// - 2: `SOURCE_SCHED_VSTP` (from a VSTP schedule)
    /// - 3: `SOURCE_TRUST` (TRUST Train Movements)
    /// - 4: `SOURCE_DARWIN` (Darwin Push Port)
    /// - 5: `SOURCE_TRUST_NAIVE` (TRUST naïve estimations)
    pub source: i32,
    /// The updated platform data, if any.
    pub platform: Option<String>,
    /// Whether or not the platform data should be suppressed (not displayed
    /// to the public) at this location.
    pub pfm_suppr: bool,
    /// Whether or not the delay should be marked as 'unknown' (i.e. the time
    /// estimated in this movement is just a best guess, and the delay cannot be
    /// accurately predicted).
    pub unknown_delay: bool
}
impl DbType for TrainMvt {
    fn table_name() -> &'static str {
        "train_movements"
    }
    fn from_row(row: &Row) -> RowResult<Self> {
        Ok(Self {
            id: row.get(0),
            parent_train: row.get(1),
            updates: row.get(2),
            tiploc: row.get(3),
            actual: row.get(4),
            time: row.get(5),
            public_time: row.get(6),
            day_offset: row.get(7),
            source: row.get(8),
            platform: row.get(9),
            pfm_suppr: row.get(10),
            unknown_delay: row.get(11)
        })
    }
}
impl InsertableDbType for TrainMvt {
    type Id = i64;
    fn insert_self(&self, conn: &Connection) -> RowResult<i64> {
        let mut stmt = conn.prepare("INSERT INTO train_movements
                                     (parent_train, updates, tiploc,
                                      actual, time, public_time,
                                      day_offset, source, platform,
                                      pfm_suppr, unknown_delay)
                                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")?;
        let rid = stmt.insert(params![self.parent_train, self.updates,
                              self.tiploc, self.actual, self.time,
                              self.public_time, self.day_offset,
                              self.course, self.platform,
                              self.pfm_suppr, self.unknown_delay])?;
        Ok(rid)
    }
}
