//! Main context for requests / responses and that

use tspl_sqlite::TsplPool;
use tspl_sqlite::traits::*;
use crate::types::{Schedule, ScheduleMvt, ScheduleDetails, ScheduleDays, MvtQueryResponse};
use crate::errors::{FahrplanResult, FahrplanError};
use std::collections::HashMap;
use tspl_sqlite::rusqlite::Connection;
use tspl_util::http::HttpServer;
use log::*;
use chrono::*;
use std::sync::{RwLock, Arc};
use rouille::{Request, Response, router};

fn get_auth_schedule(conn: &Connection, uid: String, on_date: NaiveDate, source: u8) -> FahrplanResult<Option<Schedule>> {
    debug!("Finding authoritative schedule for (uid, on_date, source) = ({}, {}, {})", uid, on_date, source);
    let scheds = Schedule::from_select(conn, "WHERE uid = ?1 AND start_date <= ?2 AND end_date >= ?2 AND source = ?3",
                                       &[&uid, &on_date, &source])?;
    let mut ret = None;
    for sched in scheds.into_iter() {
        debug!("Considering schedule #{}...", sched.id);
        if !sched.days.runs_on_iso_weekday(on_date.weekday().number_from_monday()) {
            debug!("...doesn't run on day");
            continue;
        }
        if ret.is_none() {
            debug!("...first schedule considered");
            ret = Some(sched);
            continue;
        }
        let other = ret.take().unwrap();
        if sched.stp_indicator < other.stp_indicator {
            debug!("...supersedes current schedule");
            ret = Some(sched);
        }
        else if sched.stp_indicator == other.stp_indicator && sched.stp_indicator != "C" {
            error!("Inconsistency: schedule #{} has a STP indicator equal to #{}", sched.id, other.id);
            return Err(FahrplanError::StpIndicatorsEqual);
        }
        else {
            debug!("...doesn't supersede current schedule");
            ret = Some(other);
        }
    }
    Ok(ret)
}

pub struct App {
    pub(crate) pool: Arc<RwLock<TsplPool>>,
}

impl App {
    // FIXME: This function doesn't yet handle trains crossing over midnight, like its friends in
    // the `tspl-zugfuhrer` crate.
    pub fn get_mvts_passing_through(&self, tpl: String, ts: NaiveDateTime, within_dur: Duration) -> FahrplanResult<MvtQueryResponse> {
        let (start_time, end_time) = tspl_util::time::calculate_non_midnight_aware_times(ts, within_dur);
        info!("Finding mvts passing through {} on {} between {} and {}", tpl, ts.date(), start_time, end_time);
        let db = self.pool.read().unwrap().get()?;
        let mut stmt = db.prepare("    SELECT * FROM schedule_movements AS smvts 
                                   INNER JOIN schedules AS s
                                           ON s.id = smvts.parent_sched
                              LEFT OUTER JOIN schedules AS s2
                                           ON s.uid = s2.uid
                                        WHERE smvts.tiploc = :tpl
                                          AND smvts.time BETWEEN :start_time AND :end_time
                                          AND smvts.day_offset = 0 
                                          AND :date BETWEEN s.start_date AND s.end_date
                                          AND :date BETWEEN s2.start_date AND s2.end_date
                                          AND s.days & :days
                                          AND s2.days & :days
                                          ")?;
        let days = ScheduleDays::from_iso_weekday(ts.date().weekday().number_from_monday()).unwrap();
        let args = named_params! {
            ":tpl": tpl,
            ":start_time": start_time,
            ":end_time": end_time,
            ":days": days.bits(),
            ":date": ts.date()
        };
        let rows = stmt.query_map_named(args, |row| {
            Ok((
                // The original schedule movement, passing through `tpl`.
                ScheduleMvt::from_row(row, 0)?,
                // Its parent schedule.
                Schedule::from_row(row, ScheduleMvt::FIELDS)?,
                // Another schedule which might supersede the parent, if it exists.
                Schedule::from_row(row, ScheduleMvt::FIELDS + Schedule::FIELDS).ok()
            ))
        })?;
        let mut smvts = vec![];
        let mut schedules: HashMap<String, Schedule> = HashMap::new();
        let mut proc = 0;
        for row in rows {
            let (smvt, sched, updating_sched) = row?;
            proc += 1;
            smvts.push(smvt);
            let uid = sched.uid.clone();
            if !schedules.contains_key(&uid) {
                schedules.insert(uid.clone(), sched);
            }
            if let Some(updating_sched) = updating_sched {
                let sched = schedules.get_mut(&uid).unwrap();
                if updating_sched.stp_indicator < sched.stp_indicator {
                    // The updating schedule supersedes the schedule.
                    *sched = updating_sched;
                }
            }
        }
        let mut id_schedules: HashMap<i64, Schedule> = HashMap::new();
        for (_, schedule) in schedules {
            id_schedules.insert(schedule.id, schedule);
        }
        let orig_smvts = smvts.len();
        smvts.retain(|mvt| id_schedules.contains_key(&mvt.parent_sched));
        info!("Processed {} rows for a total of {} valid smvts ({} invalid) and {} schedules.",
              proc, smvts.len(), orig_smvts - smvts.len(), id_schedules.len());
        Ok(MvtQueryResponse {
            mvts: smvts,
            schedules: id_schedules
        })
    }
    pub fn find_schedules_with_uid(&self, uid: String) -> FahrplanResult<Vec<Schedule>> {
        let db = self.pool.read().unwrap().get()?;
        let scheds = Schedule::from_select(&db, "WHERE uid = ?", &[&uid])?;
        Ok(scheds)
    }
    pub fn find_schedule_for_activation(&self, uid: String, stp_indicator: String, start_date: NaiveDate, source: u8) -> FahrplanResult<Schedule> {
        let db = self.pool.read().unwrap().get()?;
        let scheds = Schedule::from_select(&db, "WHERE uid = ? AND stp_indicator = ? AND start_date = ? AND source = ?", 
                                           &[&uid, &stp_indicator, &start_date, &source])?;
        Ok(scheds.into_iter().nth(0).ok_or(FahrplanError::NotFound)?)
    }
    pub fn find_schedule_on_date(&self, uid: String, on_date: NaiveDate, source: u8) -> FahrplanResult<Schedule> {
        let db = self.pool.read().unwrap().get()?;
        let auth_sched = get_auth_schedule(&db, uid, on_date, source)?
            .ok_or(FahrplanError::NotFound)?;
        Ok(auth_sched)
    }
    pub fn request_schedule_details(&self, uu: Uuid) -> FahrplanResult<ScheduleDetails> {
        let db = self.pool.read().unwrap().get()?;
        let scheds = Schedule::from_select(&db, "WHERE tspl_id = ?", &[&uu])?;
        let sched = scheds.into_iter().nth(0)
            .ok_or(FahrplanError::NotFound)?;
        let mvts = ScheduleMvt::from_select(&db, "WHERE parent_sched = ? ORDER BY day_offset, time, action ASC", &[&sched.id])?;
        Ok(ScheduleDetails {
            sched,
            mvts
        })
    }
}
impl HttpServer for App {
    type Error = FahrplanError;
    fn on_request(&self, req: &Request) -> FahrplanResult<Response> {
        router!(req,
            (GET) (/) => {
                Ok(Response::text(concat!("tspl-fahrplan ", env!("CARGO_PKG_VERSION"), "\n")))
            },
            (GET) (/schedules/by-uid/{uid}) => {
                self.find_schedules_with_uid(uid)
                    .map(|x| Response::json(&x))
            },
            (GET) (/schedules/by-uid-on-date/{uid}/{on_date}/{source}) => {
                self.find_schedule_on_date(uid, on_date, source)
                    .map(|x| Response::json(&x))
            },
            (GET) (/schedules/for-activation/{uid}/{start_date}/{stp_indicator}/{source}) => {
                self.find_schedule_for_activation(uid, stp_indicator, start_date, source)
                    .map(|x| Response::json(&x))
            },
            (GET) (/schedule/{uuid}) => {
                self.request_schedule_details(uuid)
                    .map(|x| Response::json(&x))
            },
            (GET) (/schedule-movements/through/{tiploc}/at/{ts: NaiveDateTime}/within-secs/{dur: u32}) => {
                self.get_mvts_passing_through(tiploc, ts, Duration::seconds(dur as _))
                    .map(|x| Response::json(&x))
            },
            _ => {
                Err(FahrplanError::NotFound)
            }
        )
    }
}
