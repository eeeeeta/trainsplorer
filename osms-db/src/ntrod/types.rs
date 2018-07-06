use db::{DbType, InsertableDbType, GenericConnection, Row};
use postgis::ewkb::Point;
use ntrod_types::schedule::*;
use ntrod_types::cif::*;
use chrono::*;
use errors::*;

pub use ntrod_types::reference::CorpusEntry;

/// A schedule from NROD, describing how trains should theoretically run.
#[derive(Debug, Serialize, Clone)]
pub struct Schedule {
    /// Internal primary key.
    pub id: i32,
    /// Schedule UID from NROD.
    pub uid: String,
    /// Schedule start date.
    pub start_date: NaiveDate,
    /// Schedule end date.
    pub end_date: NaiveDate,
    /// Days of the week where this schedule applies.
    pub days: Days,
    /// STP indicator from NROD.
    pub stp_indicator: StpIndicator,
    /// Signalling ID / headcode. Blank for freight services.
    pub signalling_id: Option<String>,
    pub geo_generation: i32,
    /// Source - one of:
    ///
    /// - 0 for schedules from CIF/ITPS
    /// - 1 for schedules from VSTP/TOPS
    pub source: i32,
    /// The sequence number of the file this was imported from,
    /// if imported from CIF/ITPS
    pub file_metaseq: Option<i32>
}
impl DbType for Schedule {
    fn table_name() -> &'static str {
        "schedules"
    }
    fn from_row(row: &Row) -> Self {
        Self {
            id: row.get(0),
            uid: row.get(1),
            start_date: row.get(2),
            end_date: row.get(3),
            days: row.get(4),
            stp_indicator: row.get(5),
            signalling_id: row.get(6),
            geo_generation: row.get(7),
            source: row.get(8),
            file_metaseq: row.get(9),
        }
    }
}
impl InsertableDbType for Schedule {
    type Id = i32;
    fn insert_self<T: GenericConnection>(&self, conn: &T) -> Result<i32> {
        let qry = conn.query(
            "INSERT INTO schedules
             (uid, start_date, end_date, days, stp_indicator, signalling_id, geo_generation, source, file_metaseq)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             RETURNING id",
            &[&self.uid, &self.start_date, &self.end_date, &self.days, &self.stp_indicator,
              &self.signalling_id, &self.geo_generation, &self.source, &self.file_metaseq])?;
        let mut ret = None;
        for row in &qry {
            ret = Some(row.get(0))
        }
        Ok(ret.expect("No id in Schedule::insert?!"))
    }
}


impl Schedule {
    /// Checks whether this schedule is authoritative on the given date.
    ///
    /// 'Authoritative' = not superseded by another schedule, like an overlay or
    /// cancellation schedule.
    pub fn is_authoritative<T: GenericConnection>(&self, conn: &T, on_date: NaiveDate) -> Result<bool> {
        if !self.days.value_for_iso_weekday(on_date.weekday().number_from_monday()).unwrap() {
            warn!("Schedule #{} was asked is_authoritative() outside days range", self.id);
            return Ok(false);
        }
        if on_date > self.end_date || on_date < self.start_date {
            warn!("Schedule #{} was asked is_authoritative() outside date range", self.id);
            return Ok(false);
        }
        debug!("Checking authoritativeness of schedule #{} on {}", self.id, on_date);
        let scheds = Schedule::from_select(conn, "WHERE uid = $1
                                                  AND start_date <= $2 AND end_date >= $2",
                                           &[&self.uid, &on_date])?;
        let mut highest = (0, StpIndicator::None, 0);
        for sched in scheds.into_iter() {
            if !sched.days.value_for_iso_weekday(on_date.weekday().number_from_monday()).unwrap() {
                continue;
            }
            if sched.stp_indicator > highest.1 {
                debug!("Schedule #{} ({:?}) supersedes schedule #{} ({:?})",
                       sched.id, sched.stp_indicator, highest.0, highest.1);
                highest = (sched.id, sched.stp_indicator, sched.source);
            }
            else if sched.stp_indicator == highest.1 && !sched.stp_indicator.is_cancellation() {
                if sched.source > highest.2 {
                    warn!("Schedule #{} ({:?}) supersedes schedule #{} ({:?}), but only because of source ({} vs {})!",
                    sched.id, sched.stp_indicator, highest.0, highest.1, sched.source, highest.2);
                    highest = (sched.id, sched.stp_indicator, sched.source);
                }
                else {
                    error!("Inconsistency: schedule #{} has a STP indicator equal to #{}",
                           sched.id, highest.0);
                    return Err(OsmsError::DatabaseInconsistency("STP indicators equal"));
                }
            }
        }
        Ok(highest.0 == self.id)
    }
}
#[derive(Debug, Serialize, Clone)]
/// Describes a movement a train makes within a schedule.
pub struct ScheduleMvt {
    /// Internal primary key.
    pub id: i32,
    /// Schedule this is a part of.
    pub parent_sched: i32,
    /// Timing Point Location where this movement happens.
    pub tiploc: String,
    /// What actually happens here - one of:
    ///
    /// - 0: arrival
    /// - 1: departure
    /// - 2: pass
    pub action: i32,
    /// Whether the train originates or terminates here.
    ///
    /// (Obviously, look at `action` to determine which)
    pub origterm: bool,
    /// The time at which this movement happens.
    pub time: NaiveTime,
    pub starts_path: Option<i32>,
    pub ends_path: Option<i32>
}
impl DbType for ScheduleMvt {
    fn table_name() -> &'static str {
        "schedule_movements"
    }
    fn from_row(row: &Row) -> Self {
        Self {
            id: row.get(0),
            parent_sched: row.get(1),
            tiploc: row.get(2),
            action: row.get(3),
            origterm: row.get(4),
            time: row.get(5),
            starts_path: row.get(6),
            ends_path: row.get(7),
        }
    }
}
impl InsertableDbType for ScheduleMvt {
    type Id = i32;
    fn insert_self<T: GenericConnection>(&self, conn: &T) -> Result<i32> {
        let qry = conn.query("INSERT INTO schedule_movements
                              (parent_sched, tiploc, action, origterm, time)
                              VALUES ($1, $2, $3, $4, $5)
                              RETURNING id",
                             &[&self.parent_sched, &self.tiploc,
                               &self.action, &self.origterm, &self.time])?;
        let mut ret = None;
        for row in &qry {
            ret = Some(row.get(0))
        }
        Ok(ret.expect("No id in ScheduleMvt::insert?!"))
    }
}
#[derive(Debug, Clone)]
/// A train, tracked in real-time.
pub struct Train {
    /// Internal primary key.
    pub id: i32,
    /// Schedule this train is running from.
    pub parent_sched: i32,
    /// TRUST ID from NROD.
    pub trust_id: String,
    /// Date this train was running on.
    pub date: NaiveDate,
    /// Signalling headcode from TRUST.
    pub signalling_id: String,
    /// Whether this train was cancelled or not.
    pub cancelled: bool,
    /// Whether this train has terminated or not.
    pub terminated: bool,
    /// A National Rail Enquiries RTTI ID for this train.
    pub nre_id: Option<String>
}
impl DbType for Train {
    fn table_name() -> &'static str {
        "trains"
    }
    fn from_row(row: &Row) -> Self {
        Self {
            id: row.get(0),
            parent_sched: row.get(1),
            trust_id: row.get(2),
            date: row.get(3),
            signalling_id: row.get(4),
            cancelled: row.get(5),
            terminated: row.get(6),
            nre_id: row.get(7)
        }
    }
}
impl InsertableDbType for Train {
    type Id = i32;
    fn insert_self<T: GenericConnection>(&self, conn: &T) -> Result<i32> {
        let qry = conn.query("INSERT INTO trains
                              (parent_sched, trust_id, date, signalling_id, cancelled, terminated, nre_id)
                              VALUES ($1, $2, $3, $4, $5, $6, $7)
                              RETURNING id",
                             &[&self.parent_sched, &self.trust_id, &self.date,
                               &self.signalling_id, &self.cancelled, &self.terminated, &self.nre_id])?;
        let mut ret = None;
        for row in &qry {
            ret = Some(row.get(0))
        }
        Ok(ret.expect("No id in Train::insert?!"))
    }
}
#[derive(Debug, Clone)]
/// A live update to a `ScheduleMvt`.
pub struct TrainMvt {
    /// Internal primary key.
    pub id: i32,
    /// References the `Train` this movement belongs to.
    pub parent_train: i32,
    /// Which schedule movement this updates.
    pub parent_mvt: i32,
    /// The updated time.
    pub time: NaiveTime,
    /// Source of this update - references the movement_sources table
    /// Could be one of:
    ///
    /// - 0: TRUST Train Movements (built into schema)
    /// - 1: Darwin Push Port (generic) (built into schema)
    /// - anything else inserted into the movement_sources table
    pub source: i32,
    /// Whether this movement is an estimation, or an actual report.
    pub estimated: bool,
}
impl DbType for TrainMvt {
    fn table_name() -> &'static str {
        "train_movements"
    }
    fn from_row(row: &Row) -> Self {
        Self {
            id: row.get(0),
            parent_train: row.get(1),
            parent_mvt: row.get(2),
            time: row.get(3),
            source: row.get(4),
            estimated: row.get(5)
        }
    }
}
impl InsertableDbType for TrainMvt {
    type Id = i32;
    fn insert_self<T: GenericConnection>(&self, conn: &T) -> Result<i32> {
        let qry = conn.query("INSERT INTO train_movements
                              (parent_train, parent_mvt, time, source, estimated)
                              VALUES ($1, $2, $3, $4, $5)
                              RETURNING id",
                             &[&self.parent_train, &self.parent_mvt, &self.time, &self.source, &self.estimated])?;
        let mut ret = None;
        for row in &qry {
            ret = Some(row.get(0))
        }
        Ok(ret.expect("No id in TrainMvt::insert?!"))
    }
}
#[derive(Debug, Clone)]
/// A description of where a live update comes from.
pub struct MvtSource {
    /// Movement source ID.
    pub id: i32,
    /// Source type - one of:
    ///
    /// - 0: Network Rail Open Data
    /// - 1: National Rail Enquiries
    pub source_type: i32,
    /// Source text - free-form text describing the data source.
    pub source_text: String
}
impl DbType for MvtSource {
    fn table_name() -> &'static str {
        "movement_sources"
    }
    fn from_row(row: &Row) -> Self {
        Self {
            id: row.get(0),
            source_type: row.get(1),
            source_text: row.get(2),
        }
    }
}
impl InsertableDbType for MvtSource {
    type Id = i32;
    fn insert_self<T: GenericConnection>(&self, conn: &T) -> Result<i32> {
        let qry = conn.query("INSERT INTO mvt_sources
                              (id, source_type, source_text)
                              VALUES ($1, $2, $3)
                              RETURNING id",
                             &[&self.id, &self.source_type, &self.source_text])?;
        let mut ret = None;
        for row in &qry {
            ret = Some(row.get(0))
        }
        Ok(ret.expect("No id in MvtSource::insert?!"))
    }
}
// NB: If you change this, change the brittle SELECT
// in the station_suggestions() function in osms-web!!
#[derive(Debug, Clone)]
pub struct MsnEntry {
    pub tiploc: String,
    pub name: String,
    pub cate: i32,
    pub crs: String,
}
impl DbType for MsnEntry {
    fn table_name() -> &'static str {
        "msn_entries"
    }
    fn from_row(row: &Row) -> Self {
        Self {
            tiploc: row.get(0),
            name: row.get(1),
            cate: row.get(2),
            crs: row.get(3),
        }
    }
}
impl InsertableDbType for MsnEntry {
    type Id = ();
    fn insert_self<T: GenericConnection>(&self, conn: &T) -> Result<()> {
        conn.execute("INSERT INTO msn_entries
                      (tiploc, name, cate, crs)
                      VALUES ($1, $2, $3, $4)",
                     &[&self.tiploc, &self.name, &self.cate, &self.crs])?;
        Ok(())
    }
}
#[derive(Debug, Clone)]
pub struct NaptanEntry {
    pub atco: String,
    pub tiploc: String,
    pub crs: String,
    pub name: String,
    pub loc: Point
}
impl DbType for NaptanEntry {
    fn table_name() -> &'static str {
        "naptan_entries"
    }
    fn from_row(row: &Row) -> Self {
        Self {
            atco: row.get(0),
            tiploc: row.get(1),
            crs: row.get(2),
            name: row.get(3),
            loc: row.get(4)
        }
    }
}
impl InsertableDbType for NaptanEntry {
    type Id = ();
    fn insert_self<T: GenericConnection>(&self, conn: &T) -> Result<()> {
        conn.execute("INSERT INTO naptan_entries
                      (atco, tiploc, crs, name, loc)
                      VALUES ($1, $2, $3, $4, $5)",
                     &[&self.atco, &self.tiploc, &self.crs, &self.name, &self.loc])?;
        Ok(())
    }
}
/*
/// A message from TRUST that we can't process yet, because we don't have the schedule data.
#[derive(Debug, Clone)]
pub struct PendingMessage {
    pub id: i32,
    /// Message type - one of:
    ///
    /// - 0: activation
    /// - 1: cancellation
    /// - 2: movement
    pub kind: i32,
    /// Which TRUST train ID this applies to.
    pub train_id: String,
    /// Date this message was received.
    pub date: NaiveDate,
    /// JSON message data.
    pub data: Value
}
impl DbType for PendingMessage {
    fn table_name() -> &'static str {
        "pending_messages"
    }
    fn table_desc() -> &'static str {
        r#"
id SERIAL PRIMARY KEY,
kind INT NOT NULL,
train_id VARCHAR NOT NULL,
date DATE NOT NULL,
data JSON NOT NULL,
UNIQUE(train_id, date)
"#
    }
    fn from_row(row: &Row) -> Self {
        Self {
            id: row.get(0),
            kind: row.get(1),
            train_id: row.get(2),
            date: row.get(3),
            data: row.get(4),
        }
    }
}
impl InsertableDbType for PendingMessage {
    type Id = ();
    fn insert_self<T: GenericConnection>(&self, conn: &T) -> Result<()> {
        conn.execute("INSERT INTO pending_messages
                      (id, kind, train_id, date, data)
                      VALUES ($1, $2, $3)",
                     &[&self.id, &self.kind, &self.train_id, &self.date, &self.data])?;
        Ok(())
    }
}
*/
#[derive(Debug, Clone)]
pub struct ScheduleFile {
    pub id: i32,
    pub timestamp: i64,
    pub metatype: String,
    pub metaseq: i32,
}
impl DbType for ScheduleFile {
    fn table_name() -> &'static str {
        "schedule_files"
    }
    fn from_row(row: &Row) -> Self {
        Self {
            id: row.get(0),
            timestamp: row.get(1),
            metatype: row.get(2),
            metaseq: row.get(3),
        }
    }
}
impl InsertableDbType for ScheduleFile {
    type Id = ();
    fn insert_self<T: GenericConnection>(&self, conn: &T) -> Result<()> {
        conn.execute("INSERT INTO schedule_files
                      (timestamp, metatype, metaseq)
                      VALUES ($1, $2, $3)",
                     &[&self.timestamp, &self.metatype, &self.metaseq])?;
        Ok(())
    }
}
impl InsertableDbType for CorpusEntry {
    type Id = ();
    fn insert_self<T: GenericConnection>(&self, conn: &T) -> Result<()> {
        conn.execute("INSERT INTO corpus_entries
                      (stanox, uic, crs, tiploc, nlc, nlcdesc, nlcdesc16)
                      VALUES ($1, $2, $3, $4, $5, $6, $7)
                      ON CONFLICT DO NOTHING",
                     &[&self.stanox, &self.uic, &self.crs, &self.tiploc, &self.nlc,
                       &self.nlcdesc, &self.nlcdesc16])?;
        Ok(())
    }
}
impl DbType for CorpusEntry {
    fn table_name() -> &'static str {
        "corpus_entries"
    }
    fn from_row(row: &Row) -> Self {
        Self {
            stanox: row.get(0),
            uic: row.get(1),
            crs: row.get(2),
            tiploc: row.get(3),
            nlc: row.get(4),
            nlcdesc: row.get(5),
            nlcdesc16: row.get(6),
        }
    }
}
#[derive(Debug, Clone)]
pub struct CrossingClosure {
    pub st: NaiveDateTime,
    pub et: NaiveDateTime,
    pub schedule_way: i32,
    pub from_sched: Option<i32>,
    pub from_train: Option<i32>
}
#[derive(Debug, Clone)]
pub struct CrossingStatus {
    pub crossing: i32,
    pub date: NaiveDate,
    pub open: bool,
    pub change_at: NaiveDateTime,
    pub closures: Vec<CrossingClosure>
}
