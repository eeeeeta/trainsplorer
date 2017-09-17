pub mod types;
pub mod live;
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
    let cur = Local::now().naive_utc();
    let sps = StationPath::from_select(conn, "WHERE $1 = ANY(crossings)", &[&cid])?;
    let mut station_paths = HashMap::new();
    let mut schedules = HashMap::new();
    let mut trains = HashMap::new();
    for path in sps {
        station_paths.insert(path.id, path);
    }
    let mut ways = vec![];
    for (id, _) in station_paths.iter() {
        ways.extend(ScheduleWay::from_select(conn, "WHERE station_path = $1
                                                    AND start_date <= $2 AND end_date >= $2
                                                    AND st <= $3 AND et >= $3",
                                                    &[&id, &cur.date(), &cur.time()])?.into_iter());
    }
    let mut ways_to_remove = vec![];
    for (i, way) in ways.iter().enumerate() {
        let schedule = Schedule::from_select(conn, "WHERE id = $1", &[&way.parent_id])?;
        if let Some(schedule) = schedule.into_iter().nth(0) {
            if !schedule.is_authoritative(conn, cur.date())? {
                ways_to_remove.insert(0, i);
            }
            schedules.insert(schedule.id, schedule);
            continue;
        }
        let train = Train::from_select(conn, "WHERE id = $1", &[&way.train_id])?;
        if let Some(train) = train.into_iter().nth(0) {
            trains.insert(train.id, train);
            continue;
        }
        bail!("Way ID {} has no associated schedule or train. Foreign keys broke.", way.id);
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
            date: cur.date(),
            open: true,
            change_at: end_of_day,
            closures: vec![]
        })
    }
    let mut closures = vec![];
    let mut change_at_open = None;
    let mut change_at_closed = None;
    for way in ways {
        let from_sched = if let Some(ref id) = way.parent_id {
            schedules.get(id)
        } else { None };
        let from_train = if let Some(ref id) = way.train_id {
            trains.get(id)
        } else { None };
        assert!(from_train.is_some() != from_sched.is_some());

        let station_path = station_paths.get(&way.station_path).unwrap();
        let pos = station_path.crossings.iter().position(|&x| x == crossing.node_id).unwrap();

        let start_dt = cur.date().and_time(way.st);

        let way_duration = way.et.signed_duration_since(way.st).num_milliseconds();
        let time_elapsed_ms =
            (way_duration as f64 * station_path.crossing_locations[pos]).trunc() as i64;
        let crossing_dt = start_dt + Duration::milliseconds(time_elapsed_ms);

        let cc = CrossingClosure {
            st: crossing_dt - Duration::minutes(1),
            et: crossing_dt + Duration::minutes(1),
            schedule_way: way.id,
            from_sched: from_sched.map(|x| x.id),
            from_train: from_train.map(|x| x.id)
        };
        match (cc.st <= cur, cc.et >= cur) {
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
    } else { change_at_open.unwrap_or(end_of_day) };
    Ok(CrossingStatus {
        crossing: crossing.node_id,
        date: cur.date(),
        open, change_at, closures
    })
}
pub fn make_schedule_ways<T: GenericConnection>(conn: &T) -> Result<()> {
    debug!("make_schedule_ways: starting...");
    let trans = conn.transaction()?;
    {
        let stmt = Schedule::prepare_select_cached(&trans, "")?;
        for sched in Schedule::from_select_iter(&trans, &stmt, &[])? {
            let sched = sched?;
            sched.make_ways(&trans)?;
        }
    }
    trans.commit()?;
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
