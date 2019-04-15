//! Database types for schedules and the like.

use tspl_sqlite::traits::*;
use tspl_sqlite::migrations::Migration;
use tspl_sqlite::migration;
use bitflags::bitflags;
use chrono::*;

pub static MIGRATIONS: [Migration; 1] = [
    migration!(0, "initial")
];
bitflags! {
    pub struct ScheduleDays: u8 {
        const MONDAY = 1;
        const TUESDAY = 2;
        const WEDNESDAY = 4;
        const THURSDAY = 8;
        const FRIDAY = 16;
        const SATURDAY = 32;
        const SUNDAY = 64;
    }
}

/// A schedule from NROD, describing how trains should theoretically run.
#[derive(Debug, Clone)]
pub struct Schedule {
    /// Internal primary key.
    pub id: i64,
    /// Schedule's trainsplorer ID.
    pub tspl_id: Uuid,
    /// Schedule UID from NROD.
    pub uid: String,
    /// Schedule start date.
    pub start_date: NaiveDate,
    /// Schedule end date.
    pub end_date: NaiveDate,
    /// Days of the week where this schedule applies.
    pub days: ScheduleDays,
    /// STP indicator from NROD.
    pub stp_indicator: String,
    /// Signalling ID / headcode. Blank for freight services.
    pub signalling_id: Option<String>,
    /// Source (see SOURCE_* associated consts)
    pub source: u8,
    /// The sequence number of the file this was imported from,
    /// if imported from CIF/ITPS
    pub file_metaseq: Option<u32>,
    /// The Darwin RID for this schedule, if obtained from Darwin.
    pub darwin_id: Option<String>,
    /// Whether or not this schedule crosses over into the next day.
    pub crosses_midnight: bool
}
impl DbType for Schedule {
    fn table_name() -> &'static str {
        "schedules"
    }
    fn from_row(row: &Row) -> RowResult<Self> {
        let days: u8 = row.get(5)?;
        Ok(Self {
            id: row.get(0)?,
            tspl_id: row.get(1)?,
            uid: row.get(2)?,
            start_date: row.get(3)?,
            end_date: row.get(4)?,
            days: ScheduleDays::from_bits_truncate(days),
            stp_indicator: row.get(6)?,
            signalling_id: row.get(7)?,
            source: row.get(8)?,
            file_metaseq: row.get(9)?,
            darwin_id: row.get(10)?,
            crosses_midnight: row.get(11)?,
        })
    }
}
impl InsertableDbType for Schedule {
    type Id = i64;
    fn insert_self(&self, conn: &Connection) -> RowResult<i64> {
        let mut stmt = conn.prepare("INSERT INTO schedules
                                     (tspl_id, uid, start_date, end_date,
                                      days, stp_indicator, signalling_id,
                                      source, file_metaseq, darwin_id, crosses_midnight)
                                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")?;
        let rid = stmt.insert(params![self.tspl_id, self.uid, self.start_date, self.end_date,
                            self.days.bits(), self.stp_indicator,
                            self.signalling_id, self.source, self.file_metaseq,
                            self.darwin_id, self.crosses_midnight])?;
        Ok(rid)
    }
}
impl Schedule {
    /// Source value for schedules from CIF/ITPS.
    pub const SOURCE_ITPS: i32 = 0;
    /// Source value for schedules from VSTP/TOPS.
    pub const SOURCE_VSTP: i32 = 1;
    /// Source value for schedules from Darwin.
    pub const SOURCE_DARWIN: i32 = 2;
} 

/// Describes a movement a train makes within a schedule.
#[derive(Debug, Clone, Hash)]
pub struct ScheduleMvt {
    /// Internal primary key.
    pub id: i64,
    /// Schedule this is a part of.
    pub parent_sched: i64,
    /// Timing Point Location where this movement happens.
    pub tiploc: String,
    /// What actually happens here - one of:
    ///
    /// - 0: arrival
    /// - 1: departure
    /// - 2: pass
    pub action: u8,
    /// The time at which this movement happens.
    pub time: NaiveTime,
    /// Day offset - number of days past the schedule start date after which this
    /// movement happens.
    ///
    /// This is used for schedules crossing midnight, which will have day_offset
    /// of 1 on the movements after midnight.
    pub day_offset: u8,
    /// Platform number this train will arrive/depart/pass at.
    pub platform: Option<String>,
    /// Public (GBTT) time for this movement - i.e. the time shown to passengers,
    /// instead of the working time.
    pub public_time: Option<NaiveTime>
}
impl ScheduleMvt {
    /// `action` value for an arrival.
    pub const ACTION_ARRIVAL: u8 = 0;
    /// `action` value for a departure.
    pub const ACTION_DEPARTURE: u8 = 1;
    /// `action` value for a pass.
    pub const ACTION_PASS: u8 = 2;
}
impl PartialEq for ScheduleMvt {
    fn eq(&self, other: &ScheduleMvt) -> bool {
        self.tiploc == other.tiploc
            && self.action == other.action
            && self.time == other.time
            && self.day_offset == other.day_offset
    }
}
impl Eq for ScheduleMvt {}
impl DbType for ScheduleMvt {
    fn table_name() -> &'static str {
        "schedule_movements"
    }
    fn from_row(row: &Row) -> RowResult<Self> {
        Ok(Self {
            id: row.get(0)?,
            parent_sched: row.get(1)?,
            tiploc: row.get(2)?,
            action: row.get(3)?,
            time: row.get(4)?,
            day_offset: row.get(5)?,
            platform: row.get(6)?,
            public_time: row.get(7)?,
        })
    }
}

/// Information on an ITPS schedule update file inserted into the database.
///
/// This is used to avoid re-inserting the same update file twice.
#[derive(Debug, Clone)]
pub struct ScheduleFile {
    /// The update file's sequence number, from its header.
    pub sequence: u32,
    /// The update file's timestamp, from its header metadata.
    pub timestamp: u32,
}
impl DbType for ScheduleFile {
    fn table_name() -> &'static str {
        "schedule_files"
    }
    fn from_row(row: &Row) -> RowResult<Self> {
        Ok(Self {
            sequence: row.get(0)?,
            timestamp: row.get(1)?
        })
    }
}
impl InsertableDbType for ScheduleFile {
    type Id = ();
    fn insert_self(&self, conn: &Connection) -> RowResult<()> {
        let mut stmt = conn.prepare("INSERT INTO schedule_files
                                     (sequence, timestamp) VALUES (?, ?)")?;
        stmt.insert(params![self.sequence, self.timestamp])?;
        Ok(())
    }
}

