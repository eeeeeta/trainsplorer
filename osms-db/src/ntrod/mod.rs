use db::{GenericConnection, DbPool, DbType, InsertableDbType};
use ntrod_types::schedule;
use ntrod_types::reference;
use serde_json;
use std::io::{Read, BufRead, BufReader};
use std::collections::HashMap;
use self::types::*;
use osm::types::{StationPath, Crossing};
use std::time::Instant;
use util::count;
use errors::*;
use chrono::*;
use std::thread;
use std::sync::atomic::{Ordering, AtomicUsize};
use std::sync::Arc;

pub mod types;

pub fn get_crossing_status<T: GenericConnection>(conn: &T, cid: i32) -> Result<CrossingStatus> {
    let crossing = Crossing::from_select(conn, "WHERE node_id = $1", &[&cid])?.into_iter()
        .nth(0).ok_or(OsmsError::CrossingNotFound(cid))?;
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
        error!("Way ID {} has no associated schedule or train. Foreign keys broke.", way.id);
        return Err(OsmsError::DatabaseInconsistency("way has no sched or train"));
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
        let pos = station_path.crossings.iter().position(|&x| x == crossing.id).unwrap();

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
        crossing: crossing.id,
        date: cur.date(),
        open, change_at, closures
    })
}
pub fn make_schedule_ways(pool: &DbPool, n_threads: usize) -> Result<()> {
    debug!("make_schedule_ways: starting, using {} threads", n_threads);
    let ways = count(&*pool.get().unwrap(), "FROM schedules WHERE processed = false", &[])?;
    debug!("make_schedule_ways: {} schedules to make ways for", ways);
    let done = Arc::new(AtomicUsize::new(0));
    let mut threads = vec![];
    for n in 0..n_threads {
        debug!("make_schedule_ways: spawning thread {}", n);
        let p = pool.clone();
        let d = done.clone();
        threads.push(thread::spawn(move || {
            let db = p.get().unwrap();
            loop {
                let trans = db.transaction().unwrap();
                let scheds = Schedule::from_select(&trans, "WHERE processed = false LIMIT 1
                                                            FOR UPDATE SKIP LOCKED", &[])
                    .unwrap();
                if scheds.len() == 0 {
                    debug!("make_schedule_ways: thread {} done", n);
                    break;
                }
                for sched in scheds {
                    let instant = Instant::now();
                    sched.make_ways(&trans).unwrap();
                    let now = Instant::now();
                    let dur = now.duration_since(instant);
                    let dur = dur.as_secs() as f64 + dur.subsec_nanos() as f64 * 1e-9;
                    let done = d.fetch_add(1, Ordering::SeqCst) + 1;
                    debug!("make_schedule_ways: {} of {} schedules complete ({:.01}%) - time: {:.04}s", done, ways, (done as f64 / ways as f64) * 100.0, dur);
                }
                trans.commit().unwrap();
            }
        }));
    }
    for thr in threads {
        thr.join().unwrap();
    }
    debug!("make_schedule_ways: complete!");
    Ok(())
}
pub fn apply_schedule_records<T: GenericConnection, R: Read>(conn: &T, rdr: R, restrict_atoc: Option<&str>) -> Result<()> {
    debug!("apply_schedule_records: running...");
    let mut inserted = 0;
    let trans = conn.transaction()?;
    let rdr = BufReader::new(rdr);
    let mut verified = false;
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
                if !verified {
                    error!("apply_schedule_records: file contained no Timetable record!");
                    return Err(OsmsError::InvalidScheduleFile);
                }
                if let Some(atc) = restrict_atoc {
                    if !rec.atoc_code.as_ref().map(|x| x == atc).unwrap_or(false) {
                        continue;
                    }
                }
                Schedule::apply_rec(&trans, rec)?;
                inserted += 1;
            },
            schedule::Record::Timetable(rec) => {
                debug!("apply_schedule_records: this is a {}-type timetable (seq {}) from {} (ts: {})",
                       rec.metadata.ty, rec.metadata.sequence, rec.owner, rec.timestamp);
                debug!("apply_schedule_records: checking whether this timetable is new...");
                let files = ScheduleFile::from_select(&trans, "WHERE timestamp = $1", &[&(rec.timestamp as i64)])?;
                if files.len() > 0 {
                    error!("apply_schedule_records: schedule inserted already!");
                    return Err(OsmsError::ScheduleFileExists);
                }
                let full = ScheduleFile::from_select(&trans, "WHERE metaseq > $1", &[&(rec.metadata.sequence as i32)])?;
                if full.len() > 0 && rec.metadata.ty == "full" {
                    error!("apply_schedule_records: a schedule with a greater sequence number has been inserted!");
                    return Err(OsmsError::ScheduleFileImportInvalid("sequence number error"));
                }
                debug!("apply_schedule_records: inserting file record...");
                let file = ScheduleFile {
                    id: -1,
                    timestamp: rec.timestamp as _,
                    metatype: rec.metadata.ty.clone(),
                    metaseq: rec.metadata.sequence as _
                };
                file.insert_self(&trans)?;
                debug!("apply_schedule_records: timetable OK");
                verified = true;
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
