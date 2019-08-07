//! Main context for requests / responses and that

use tspl_sqlite::TsplPool;
use tspl_sqlite::traits::*;
use crate::download::JobType;
use crate::dl_scheduler::DlSender;
use crate::types::{Schedule, ScheduleMvt, ScheduleDetails};
use crate::errors::{FahrplanResult, FahrplanError};
use tspl_sqlite::rusqlite::Connection;
use tspl_util::http::HttpServer;
use log::*;
use chrono::*;
use rouille::{Request, Response, router};
use std::sync::Mutex;

fn get_auth_schedule(conn: &Connection, uid: String, on_date: NaiveDate, source: u8) -> FahrplanResult<Option<Schedule>> {
    debug!("Finding authoritative schedule for (uid, on_date, source) = ({}, {}, {})", uid, on_date, source);
    let scheds = Schedule::from_select(conn, "WHERE uid = $1 AND start_date <= $2 AND end_date >= $2 AND source = $3",
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
    pub(crate) pool: TsplPool,
    pub(crate) dls_tx: Mutex<DlSender>
}

impl App {
    pub fn find_schedules_with_uid(&self, uid: String) -> FahrplanResult<Vec<Schedule>> {
        let db = self.pool.get()?;
        let scheds = Schedule::from_select(&db, "WHERE uid = ?", &[&uid])?;
        Ok(scheds)
    }
    pub fn find_schedule_for_activation(&self, uid: String, stp_indicator: String, start_date: NaiveDate, source: u8) -> FahrplanResult<Schedule> {
        let db = self.pool.get()?;
        let scheds = Schedule::from_select(&db, "WHERE uid = ?, stp_indicator = ?, start_date = ?, source = ?", 
                                           &[&uid, &stp_indicator, &start_date, &source])?;
        Ok(scheds.into_iter().nth(0).ok_or(FahrplanError::NotFound)?)
    }
    pub fn find_schedule_on_date(&self, uid: String, on_date: NaiveDate, source: u8) -> FahrplanResult<Schedule> {
        let db = self.pool.get()?;
        let auth_sched = get_auth_schedule(&db, uid, on_date, source)?
            .ok_or(FahrplanError::NotFound)?;
        Ok(auth_sched)
    }
    pub fn request_schedule_details(&self, uu: Uuid) -> FahrplanResult<ScheduleDetails> {
        let db = self.pool.get()?;
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
            (POST) (/update/{jt: JobType}) => {
                self.dls_tx
                    .lock().unwrap()
                    .send(jt)
                    .map(|_| Response::json(&()))
                    .map_err(|_| FahrplanError::JobQueueError)
            },
            _ => {
                Err(FahrplanError::NotFound)
            }
        )
    }
}
