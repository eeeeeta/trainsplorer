pub mod types;

use db::*;
use chrono::{NaiveTime, NaiveDate};
use ntrod_types::reference::CorpusEntry;
use self::types::{TrainMvt, ScheduleMvt, Schedule, Train, MvtSource};
use std::collections::HashMap;

pub fn get_stanox_for_tiploc<T: GenericConnection>(conn: &T, tiploc: &str) -> Result<Option<String>, ::failure::Error> {
    let entries = CorpusEntry::from_select(conn, "WHERE tiploc = $1 AND stanox IS NOT NULL", &[&tiploc])?;
    for entry in entries {
        return Ok(Some(entry.stanox.unwrap()));
    }
    Ok(None)
}
/// A time with corresponding source information.
///
/// See the docs of the containing struct to figure out what the fields mean.
pub struct TimeWithSource {
    pub source: i32,
    pub mvt_id: i32,
    pub time: NaiveTime
}
/// A deduplicated movement, incorporating data from a schedule and a train movement.
pub struct DeduplicatedMvt {
    pub parent_sched: i32,
    pub parent_train: Option<i32>,
    pub tiploc: String,
    pub action: i32,
    /// The scheduled time for this movement - i.e. the one in the Darwin/TRUST schedule.
    /// 
    /// - `source` is the ID of the **schedule** source that generated the time
    /// - `smvt_id` is the ID of the schedule movement
    /// - `time` is the actual time
    pub time_scheduled: TimeWithSource,
    /// The estimated time for this movement - i.e. Darwin's prediction.
    ///
    /// - `source` is the ID of the **movement** source that generated the time
    ///   (i.e. was it TRUST predictions, or Darwin predictions?)
    /// - `tmvt_id` is the ID of the train movement
    /// - `time` is the actual time
    pub time_expected: Option<TimeWithSource>,
    /// The actual time for this movement - i.e. when it actually happened, from either Darwin or
    /// TRUST.
    /// 
    /// - `source` is the ID of the **movement** source that generated the time
    ///   (i.e. was it TRUST movements, or Darwin?)
    /// - `tmvt_id` is the ID of the train movement
    /// - `time` is the actual time
    pub time_actual: Option<TimeWithSource>,
    /// Is this movement cancelled?
    pub canx: bool,
    pub starts_path: Option<i32>,
    pub ends_path: Option<i32>,
}
pub struct MvtQueryResult {
    pub mvts: Vec<DeduplicatedMvt>,
    pub scheds: HashMap<i32, Schedule>,
    pub trains: HashMap<i32, Train>,
    pub scheds_to_trains: HashMap<i32, i32>
}
pub fn mvt_query<T: GenericConnection>(conn: &T, mvts: &[ScheduleMvt], auth_date: Option<NaiveDate>) -> Result<MvtQueryResult, ::failure::Error> {
    // part I: get the relevant schedules, and check their authoritativeness
    let mut scheds = HashMap::new();
    for mvt in mvts.iter() {
        if scheds.get(&mvt.parent_sched).is_none() {
            let sched = Schedule::from_select(conn, "WHERE id = $1 AND start_date <= $2 AND end_date >= $2", &[&mvt.parent_sched, &auth_date])?
                .into_iter().nth(0).unwrap();
            if let Some(date) = auth_date {
                if !sched.is_authoritative(conn, date)? {
                    continue;
                }
            }
            scheds.insert(sched.id, sched);
        }
    }
    // part II: get the relevant trains
    // (obviously this only works if we have a date)
    let mut trains = HashMap::new();
    let mut scheds_to_trains = HashMap::new();
    if let Some(date) = auth_date {
        let sched_ids = scheds.keys().collect::<Vec<_>>();
        for train in Train::from_select(conn, "WHERE (parent_sched = ANY($1) AND date = $2) OR parent_nre_sched = ANY($1)", &[&sched_ids, &date])? {
            scheds_to_trains.insert(train.parent_sched, train.id);
            if let Some(ns) = train.parent_nre_sched {
                scheds_to_trains.insert(ns, train.id);
            }
            trains.insert(train.id, train);
        }
    }
    // part III: process each movement
    let mut ret = vec![];
    for mvt in mvts {
        let parent_sched = match scheds.get(&mvt.parent_sched) {
            Some(ps) => ps,
            None => {
                // If we couldn't find the schedule, it wasn't authoritative
                // when we looked for it earlier.
                continue;
            }
        };
        let time_scheduled = TimeWithSource {
            source: parent_sched.source,
            mvt_id: mvt.id,
            time: mvt.time
        };
        let mut time_expected: Option<TimeWithSource> = None;
        let mut time_actual: Option<TimeWithSource> = None;
        let mut ptid = None;
        let mut canx = false;

        // do we have a train associated with this schedule?
        if let Some(&parent_train_id) = scheds_to_trains.get(&mvt.parent_sched) {
            ptid = Some(parent_train_id);
            let parent_train = &trains[&parent_train_id];
            let tmvts = TrainMvt::from_select(conn, "WHERE parent_train = $1 AND parent_mvt = $2", &[&parent_train_id, &mvt.id])?;
            // if the parent schedule is a Darwin schedule, it needs to have a corresponding
            // train movement, otherwise it gets rejected
            // (because Darwin schedules shouldn't be used without tmvts)
            if parent_sched.darwin_id.is_some() && tmvts.len() == 0 {
                continue;
            }
            // fill in actual/expected (i.e. 'live') times from the tmvts
            for tmvt in tmvts {
                if tmvt.estimated {
                    if let Some(ref exi) = time_expected {
                        if exi.source == MvtSource::SOURCE_DARWIN {
                            // Darwin estimations take priority over all other kinds.
                            continue;
                        }
                    }
                    time_expected = Some(TimeWithSource {
                        source: tmvt.source,
                        mvt_id: tmvt.id,
                        time: tmvt.time
                    });
                }
                else {
                    if let Some(ref exi) = time_actual {
                        if exi.source == MvtSource::SOURCE_TRUST {
                            // TRUST movements take priority over all other actual times.
                            continue;
                        }
                    }
                    time_actual = Some(TimeWithSource {
                        source: tmvt.source,
                        mvt_id: tmvt.id,
                        time: tmvt.time
                    });
                }
            }
            if parent_train.cancelled {
                canx = true;
            }
        }
        else {
            // Check that we aren't letting any Darwin schedules through,
            // which would happen if we didn't have an auth_date.
            if parent_sched.darwin_id.is_some() {
                continue;
            }
        }
        ret.push(DeduplicatedMvt {
            parent_sched: parent_sched.id,
            parent_train: ptid,
            tiploc: mvt.tiploc.clone(),
            action: mvt.action,
            time_scheduled,
            time_expected,
            time_actual,
            canx,
            starts_path: mvt.starts_path,
            ends_path: mvt.ends_path
        });
    }
    Ok(MvtQueryResult {
        mvts: ret,
        scheds,
        trains,
        scheds_to_trains
    })
}
