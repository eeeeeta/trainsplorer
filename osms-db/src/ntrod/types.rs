use db::{DbType, InsertableDbType, GenericConnection, Row};
use osm::types::Station;
use ntrod_types::schedule::*;
use ntrod_types::reference::*;
use ntrod_types::cif::*;
use chrono::*;
use osm;
use errors::*;

#[derive(Debug, Clone)]
pub struct Schedule {
    pub id: i32,
    pub uid: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub days: Days,
    pub stp_indicator: StpIndicator,
    pub signalling_id: Option<String>,
    pub locs: Vec<ScheduleLocation>,
    pub processed: bool
}
impl Schedule {
    pub fn is_authoritative<T: GenericConnection>(&self, conn: &T, on_date: NaiveDate) -> Result<bool> {
        if on_date > self.end_date || on_date < self.start_date {
            warn!("Schedule #{} was asked is_authoritative() outside date range", self.id);
            return Ok(false);
        }
        let scheds = Schedule::from_select(conn, "WHERE uid = $1
                                                  AND start_date <= $2 AND end_date >= $2",
                                           &[&self.uid, &on_date])?;
        let mut highest = (self.id, self.stp_indicator);
        for sched in scheds {
            let trains = Train::from_select(conn, "WHERE from_id = $1
                                                   AND date = $2",
                                            &[&sched.id, &on_date])?;
            if trains.len() > 0 {
                return Ok(false);
            }
            if sched.id == self.id {
                continue;
            }
            if sched.stp_indicator > highest.1 {
                highest = (sched.id, sched.stp_indicator);
            }
            else if sched.stp_indicator == highest.1{
                error!("Inconsistency: schedule #{} has a STP indicator equal to #{}",
                       sched.id, highest.0);
                bail!("STP indicator inconsistency");
            }
        }
        Ok(highest.0 != self.id)
    }
    pub fn make_ways<T: GenericConnection>(&self, conn: &T) -> Result<()> {
        debug!("making ways for record (UID {}, start {}, stp_indicator {:?})",
               self.uid, self.start_date, self.stp_indicator);
        if self.processed {
            let n_ways = ScheduleWay::from_select(conn, "WHERE parent_id = $1", &[&self.id])?.len();
            warn!("Already processed this record - has {} ways!", n_ways);
            return Ok(());
        }
        let mut p1 = 0;
        'outer: loop {
            if p1 >= self.locs.len() { break; }
            if let Some(e1) = self.locs[p1].get_station(conn)? {
                for p2 in p1..self.locs.len() {
                    if let Some(e2) = self.locs[p2].get_station(conn)? {
                        if e1.nr_ref == e2.nr_ref {
                            continue;
                        }
                        let path = match osm::navigate::navigate_cached(conn, &e1.nr_ref, &e2.nr_ref) {
                            Ok(x) => x,
                            Err(e) => {
                                error!("*** failed to navigate from {} to {}: {} ***",
                                       e1.nr_ref, e2.nr_ref, e);
                                continue;
                            }
                        };
                        debug!("made way from {} ({}) to {} ({})",
                               e1.nr_ref, self.locs[p1].time,
                               e2.nr_ref, self.locs[p2].time);
                        let sway = ScheduleWay {
                            st: self.locs[p1].time,
                            et: self.locs[p2].time,
                            start_date: self.start_date,
                            end_date: self.end_date,
                            station_path: path,
                            id: -1,
                            parent_id: Some(self.id),
                            train_id: None
                        };
                        sway.insert_self(conn)?;
                        p1 = p2 + 1;
                        continue 'outer;
                    }
                }
            }
            p1 += 1;
        }
        conn.execute("UPDATE schedules SET processed = true WHERE id = $1", &[&self.id])?;
        Ok(())
    }
    pub fn apply_rec<T: GenericConnection>(conn: &T, rec: ScheduleRecord) -> Result<Option<i32>> {
        use self::LocationRecord::*;
        if let CreateOrDelete::Delete = rec.transaction_type {
            conn.execute("DELETE FROM schedules
                          WHERE uid = $1 AND start_date = $2 AND stp_indicator = $3",
                         &[&rec.train_uid, &rec.schedule_start_date.naive_utc(), &rec.stp_indicator])?;
            return Ok(None);
        }
        if let YesOrNo::N = rec.applicable_timetable {
            return Ok(None);
        }
        debug!("inserting record (UID {}, start {}, stp_indicator {:?})",
               rec.train_uid, rec.schedule_start_date, rec.stp_indicator);
        let ScheduleRecord {
            train_uid,
            schedule_start_date,
            schedule_end_date,
            schedule_days_runs,
            stp_indicator,
            schedule_segment,
            ..
        } = rec;
        let ScheduleSegment {
            schedule_location,
            signalling_id,
            ..
        } = schedule_segment;
        let mut sched = Schedule {
            uid: train_uid,
            start_date: schedule_start_date.naive_utc(),
            end_date: schedule_end_date.naive_utc(),
            days: schedule_days_runs,
            stp_indicator,
            signalling_id,
            processed: false,
            locs: vec![],
            id: -1
        };
        for loc in schedule_location {
            match loc {
                Originating { tiploc_code, departure, .. } => {
                    sched.locs.push(
                        ScheduleLocation::new(tiploc_code, departure, "originate"));
                },
                Intermediate { tiploc_code, arrival, departure, .. } => {
                    sched.locs.push(
                        ScheduleLocation::new(tiploc_code.clone(), arrival, "arrive"));
                    sched.locs.push(
                        ScheduleLocation::new(tiploc_code, departure, "depart"));
                },
                Pass { tiploc_code, pass, .. } => {
                    sched.locs.push(
                        ScheduleLocation::new(tiploc_code, pass, "pass"));
                },
                Terminating { tiploc_code, arrival, .. } => {
                    sched.locs.push(
                        ScheduleLocation::new(tiploc_code, arrival, "terminate"));
                }
            }
        }
        Ok(Some(sched.insert_self(conn)?))
    }
}
impl InsertableDbType for Schedule {
    type Id = i32;
    fn insert_self<T: GenericConnection>(&self, conn: &T) -> Result<i32> {
        for row in &conn.query(
            "SELECT id FROM schedules
             WHERE uid = $1 AND start_date = $2 AND stp_indicator = $3",
                               &[&self.uid, &self.start_date, &self.stp_indicator])? {
            return Ok(row.get(0));
        }
        let qry = conn.query(
            "INSERT INTO schedules
             (uid, start_date, end_date, days, stp_indicator, signalling_id, locs, processed)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING id",
            &[&self.uid, &self.start_date, &self.end_date, &self.days, &self.stp_indicator,
              &self.signalling_id, &self.locs, &self.processed])?;
        let mut ret = None;
        for row in &qry {
            ret = Some(row.get(0))
        }
        Ok(ret.expect("No id in Schedule::insert?!"))
    }
}
impl DbType for Schedule {
    fn table_name() -> &'static str {
        "schedules"
    }
    fn table_desc() -> &'static str {
        r#"
id SERIAL PRIMARY KEY,
uid VARCHAR NOT NULL,
start_date DATE NOT NULL,
end_date DATE NOT NULL,
days "Days" NOT NULL,
stp_indicator "StpIndicator" NOT NULL,
signalling_id VARCHAR,
locs "ScheduleLocation"[] NOT NULL,
processed BOOL NOT NULL,
UNIQUE(uid, start_date, stp_indicator)
"#
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
            locs: row.get(7),
            processed: row.get(8)
        }
    }
}

#[derive(Debug, ToSql, FromSql, Clone)]
pub struct ScheduleLocation {
    pub tiploc: String,
    pub time: NaiveTime,
    pub event: String
}
impl ScheduleLocation {
    pub fn create_type() -> &'static str {
        r#"
DO $$
BEGIN
IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'ScheduleLocation') THEN
CREATE TYPE "ScheduleLocation" AS (
tiploc VARCHAR,
time TIME,
event VARCHAR
);
END IF;
END$$;"#
    }
    pub fn new<T: Into<String>, U: Into<String>>(tiploc: T, dep: NaiveTime, event: U) -> Self {
        Self { tiploc: tiploc.into(), time: dep, event: event.into() }
    }
    pub fn get_station<T: GenericConnection>(&self, conn: &T) -> Result<Option<Station>> {
        Ok(if let Some(crs) = self.get_crs(conn)? {
            let stats = Station::from_select(conn, "WHERE nr_ref = $1", &[&crs])?;
            if stats.len() == 0 {
                trace!("No station for CRS {}", crs);
            }
            stats.into_iter()
                .nth(0)
        }
        else {
            None
        })
    }
    pub fn get_crs<T: GenericConnection>(&self, conn: &T) -> Result<Option<String>> {
        let entries = CorpusEntry::from_select(conn,
                                               "WHERE tiploc = $1 AND crs IS NOT NULL",
                                               &[&self.tiploc])?;
        let mut ret = None;
        for ent in entries {
            if ent.crs.is_some() {
                ret = ent.crs;
                break;
            }
        }
        if ret.is_none() {
            trace!("Could not find a CRS for TIPLOC {}", self.tiploc);
        }
        Ok(ret)
    }
}
#[derive(Debug, Clone)]
pub struct Train {
    pub id: i32,
    pub from_id: i32,
    pub trust_id: String,
    pub date: NaiveDate,
    pub signalling_id: String,
}
impl DbType for Train {
    fn table_name() -> &'static str {
        "trains"
    }
    fn table_desc() -> &'static str {
        r#"
id SERIAL PRIMARY KEY,
from_id INT NOT NULL REFERENCES schedules ON DELETE CASCADE,
trust_id VARCHAR NOT NULL UNIQUE,
date DATE NOT NULL,
signalling_id VARCHAR NOT NULL
"#
    }
    fn from_row(row: &Row) -> Self {
        Self {
            id: row.get(0),
            from_id: row.get(1),
            trust_id: row.get(2),
            date: row.get(3),
            signalling_id: row.get(4),
        }
    }
}
impl InsertableDbType for Train {
    type Id = i32;
    fn insert_self<T: GenericConnection>(&self, conn: &T) -> Result<i32> {
        let qry = conn.query("INSERT INTO trains
                              (from_id, trust_id, date, signalling_id)
                              VALUES ($1, $2, $3, $4)
                              RETURNING id",
                             &[&self.from_id, &self.trust_id, &self.date,
                               &self.signalling_id])?;
        let mut ret = None;
        for row in &qry {
            ret = Some(row.get(0))
        }
        Ok(ret.expect("No id in Train::insert?!"))
    }
}

#[derive(Debug, Clone)]
pub struct ScheduleWay {
    pub id: i32,
    pub parent_id: Option<i32>,
    pub train_id: Option<i32>,
    pub st: NaiveTime,
    pub et: NaiveTime,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub station_path: i32,
}
impl DbType for ScheduleWay {
    fn table_name() -> &'static str {
        "schedule_ways"
    }
    fn table_desc() -> &'static str {
        r#"
id SERIAL PRIMARY KEY,
parent_id INT REFERENCES schedules ON DELETE CASCADE,
train_id INT REFERENCES trains ON DELETE CASCADE,
st TIME NOT NULL,
et TIME NOT NULL,
start_date DATE NOT NULL,
end_date DATE NOT NULL,
station_path INT NOT NULL REFERENCES station_paths ON DELETE RESTRICT,
CHECK((parent_id IS NULL) != (train_id IS NULL))
"#
    }
    fn from_row(row: &Row) -> Self {
        Self {
            id: row.get(0),
            parent_id: row.get(1),
            train_id: row.get(2),
            st: row.get(3),
            et: row.get(4),
            start_date: row.get(5),
            end_date: row.get(6),
            station_path: row.get(7),
        }
    }
}
impl InsertableDbType for ScheduleWay {
    type Id = i32;
    fn insert_self<T: GenericConnection>(&self, conn: &T) -> Result<i32> {
        let qry = conn.query("INSERT INTO schedule_ways
                              (st, et, station_path, parent_id, start_date, end_date)
                              VALUES ($1, $2, $3, $4, $5, $6)
                              ON CONFLICT(id) DO UPDATE SET station_path = excluded.station_path
                              RETURNING id",
                             &[&self.st, &self.et, &self.station_path,
                               &self.parent_id, &self.start_date,
                               &self.end_date])?;
        let mut ret = None;
        for row in &qry {
            ret = Some(row.get(0))
        }
        Ok(ret.expect("no ID in ScheduleWay insert"))
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
    fn table_desc() -> &'static str {
        r#"
stanox VARCHAR UNIQUE,
uic VARCHAR UNIQUE,
crs VARCHAR UNIQUE,
tiploc VARCHAR UNIQUE,
nlc VARCHAR UNIQUE,
nlcdesc VARCHAR,
nlcdesc16 VARCHAR
"#
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
