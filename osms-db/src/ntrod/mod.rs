pub mod types;
use db::{GenericConnection, DbType, InsertableDbType};
use ntrod_types::schedule;
use ntrod_types::reference;
use serde_json;
use std::io::{Read, BufRead};
use std::collections::HashMap;
use self::types::*;
use osm::types::{StationPath, Crossing};
use errors::*;
use chrono::*;

pub fn get_crossing_status<T: GenericConnection>(conn: &T, cid: i32) -> Result<CrossingStatus> {
    let crossing = Crossing::from_select(conn, "WHERE node_id = $1", &[&cid])?.into_iter()
        .nth(0).ok_or("No such crossing")?;
    let dt = Local::now().naive_utc();
    let date = dt.date();
    let time = dt.time();
    let sps = StationPath::from_select(conn, "WHERE $1 = ANY(crossings)", &[&cid])?;
    let mut station_paths = HashMap::new();
    let mut schedules = HashMap::new();
    for path in sps {
        station_paths.insert(path.id, path);
    }
    let mut ways = vec![];
    for (id, _) in station_paths.iter() {
        ways.extend(ScheduleWay::from_select(conn, "WHERE station_path = $1
                                                    AND start_date <= $2 AND end_date >= $2
                                                    AND st <= $3 AND et >= $3",
                                                    &[&id, &date, &time])?.into_iter());
    }
    let mut ways_to_remove = vec![];
    for (i, way) in ways.iter().enumerate() {
        let schedule = Schedule::from_select(conn, "WHERE id = $1", &[&way.parent_id])?;
        let schedule = schedule.into_iter().nth(0).expect("Foreign key didn't do its job");
        if schedule.higher_schedule(conn, date)?.is_some() {
            ways_to_remove.insert(0, i);
        }
        schedules.insert(schedule.id, schedule);
    }
    for idx in ways_to_remove {
        ways.remove(idx);
    }
    let end_of_day = Local::now().naive_utc()
        .with_hour(23).unwrap()
        .with_minute(59).unwrap()
        .with_second(59).unwrap();
    if ways.len() == 0 {
        return Ok(CrossingStatus {
            crossing: cid,
            date: date,
            open: true,
            change_at: end_of_day.time(),
            closures: vec![]
        })
    }
    let mut closures = vec![];
    let mut change_at_open = None;
    let mut change_at_closed = None;
    for way in ways {
        let parent_sched = schedules.get(&way.parent_id).unwrap();
        let station_path = station_paths.get(&way.station_path).unwrap();
        let pos = station_path.crossings.iter().position(|&x| x == crossing.node_id).unwrap();
        let dt = way.et.signed_duration_since(way.st).num_milliseconds();
        let time_in_ms = (dt as f64 * station_path.crossing_locations[pos]).trunc() as i64;
        let time_in = way.st.overflowing_add_signed(Duration::milliseconds(time_in_ms)).0;
        let cc = CrossingClosure {
            st: time_in.overflowing_sub_signed(Duration::minutes(1)).0,
            et: time_in.overflowing_add_signed(Duration::minutes(1)).0,
            schedule_way: way.id,
            from_uid: parent_sched.uid.clone()
        };
        match (cc.st <= time, cc.et >= time) {
            (true, true) => {
                if change_at_closed.map(|x| x > cc.et).unwrap_or(true) {
                    change_at_closed = Some(cc.et);
                }
            },
            (false, true) => {
                if change_at_open.map(|x| x > cc.st).unwrap_or(true) {
                    change_at_open = Some(cc.st);
                }
            },
            _ => {}
        }
        closures.push(cc);
    }
    let mut open = true;
    let change_at = if let Some(ca) = change_at_closed {
        open = false;
        ca
    } else { change_at_open.unwrap_or(end_of_day.time()) };
    Ok(CrossingStatus {
        crossing: crossing.node_id,
        date: date,
        open, change_at, closures
    })
}
pub fn make_schedule_ways<T: GenericConnection>(conn: &T) -> Result<()> {
    debug!("make_schedule_ways: getting schedules...");
    let scheds = Schedule::from_select(conn, "", &[])?;
    debug!("make_schedule_ways: {} schedules to update", scheds.len());
    for sched in scheds {
        let trans = conn.transaction()?;
        sched.make_ways(&trans)?;
        trans.commit()?;
    }
    debug!("make_schedule_ways: complete!");
    Ok(())
}
pub fn apply_schedule_records<T: GenericConnection, R: BufRead>(conn: &T, rdr: R, restrict_atoc: Option<&str>) -> Result<()> {
    debug!("apply_schedule_records: running...");
    let mut inserted = 0;
    let trans = conn.transaction()?;
    for line in rdr.lines() {
        let line = line?;
        let rec: ::std::result::Result<schedule::Record, _> = serde_json::from_str(&line);
        let rec = match rec {
            Ok(r) => r,
            Err(e) => {
                warn!("apply_schedule_records: error parsing: {}", e);
                debug!("apply_schedule_records: line was: {}", line);
                continue;
            }
        };
        match rec {
            schedule::Record::Schedule(rec) => {
                if let Some(atc) = restrict_atoc {
                    if !rec.atoc_code.as_ref().map(|x| x == atc).unwrap_or(false) {
                        continue;
                    }
                }
                Schedule::apply_rec(&trans, rec)?;
                inserted += 1;
            },
            schedule::Record::Timetable(rec) => {
                debug!("apply_schedule_records: this is a {}-type timetable from {} (ts: {})",
                       rec.classification, rec.owner, rec.timestamp);
            },
            _ => {}
        }
    }
    trans.commit()?;
    debug!("apply_schedule_record: applied {} entries", inserted);
    Ok(())
}
pub fn import_corpus<T: GenericConnection, R: Read>(conn: &T, rdr: R) -> Result<()> {
    debug!("import_corpus: loading data from file...");
    let data: reference::CorpusData = serde_json::from_reader(rdr)?;
    debug!("import_corpus: inserting data into database...");
    let mut inserted = 0;
    let trans = conn.transaction()?;
    for ent in data.tiploc_data {
        if ent.contains_data() {
            ent.insert_self(&trans)?;
            inserted += 1;
        }
    }
    trans.commit()?;
    debug!("import_corpus: inserted {} entries", inserted);
    Ok(())
}
