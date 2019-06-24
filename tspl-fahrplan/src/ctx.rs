//! Main context for requests / responses and that

use tspl_sqlite::TsplPool;
use tspl_sqlite::traits::*;
use crate::dl_scheduler::DlSender;
use crate::proto::{FahrplanRequest, FahrplanResponse, FahrplanError, FahrplanResult, ScheduleDetails};
use crate::types::{Schedule, ScheduleMvt};
use ::serde::Serialize;
use crate::errors::Result;
use std::convert::TryInto;
use tspl_sqlite::rusqlite::Connection;
use log::*;
use chrono::*;
use failure::format_err;

pub trait FahrplanResultExt {
    fn collapse(self) -> Result<FahrplanResponse<'static>>;
}
impl<T: Serialize> FahrplanResultExt for Result<FahrplanResult<T>> {
    fn collapse(self) -> Result<FahrplanResponse<'static>> {
        match self {
            Ok(r) => Ok(r.try_into()?),
            Err(e) => {
                warn!("Error processing request: {}", e);
                let res: ::std::result::Result<(), FahrplanError> = Err(FahrplanError::InternalError(e.to_string()));
                Ok(res.try_into()?)
            }
        }
    }
}

fn get_auth_schedule(conn: &Connection, uid: String, on_date: NaiveDate, source: u8) -> Result<Option<Schedule>> {
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
            return Err(format_err!("STP indicators equal"));
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
    pub(crate) dls_tx: DlSender
}

impl App {
    pub fn find_schedules_with_uid(&mut self, uid: String) -> Result<Vec<Schedule>> {
        let db = self.pool.get()?;
        let scheds = Schedule::from_select(&db, "WHERE uid = ?", &[&uid])?;
        Ok(scheds)
    }
    pub fn find_schedule_for_activation(&mut self, uid: String, stp_indicator: String, start_date: NaiveDate, source: u8) -> Result<FahrplanResult<Schedule>> {
        let db = self.pool.get()?;
        let scheds = Schedule::from_select(&db, "WHERE uid = ?, stp_indicator = ?, start_date = ?, source = ?", 
                                           &[&uid, &stp_indicator, &start_date, &source])?;
        Ok(scheds.into_iter().nth(0).ok_or(FahrplanError::NotFound))
    }
    pub fn find_schedule_on_date(&mut self, uid: String, on_date: NaiveDate, source: u8) -> Result<FahrplanResult<Schedule>> {
        let db = self.pool.get()?;
        let auth_sched = get_auth_schedule(&db, uid, on_date, source)?.ok_or(FahrplanError::NotFound);
        Ok(auth_sched)
    }
    pub fn request_schedule_details(&mut self, uu: Uuid) -> Result<FahrplanResult<ScheduleDetails>> {
        let db = self.pool.get()?;
        let scheds = Schedule::from_select(&db, "WHERE tspl_id = ?", &[&uu])?;
        let sched = match scheds.into_iter().nth(0) {
            Some(s) => s,
            None => return Ok(Err(FahrplanError::NotFound))
        };
        let mvts = ScheduleMvt::from_select(&db, "WHERE parent_sched = ? ORDER BY day_offset, time, action ASC", &[&sched.id])?;
        Ok(Ok(ScheduleDetails {
            sched,
            mvts
        }))
    }
    pub fn process_request(&mut self, req: FahrplanRequest) -> Result<FahrplanResponse<'static>> {
        use self::FahrplanRequest::*;
        match req {
            FindSchedulesWithUid { uid } => {
                let ret = self.find_schedules_with_uid(uid)
                    .map_err(|e| FahrplanError::InternalError(e.to_string()))
                    .try_into()?;
                Ok(ret)
            },
            FindScheduleOnDate { uid, on_date, source } => {
                let ret = self.find_schedule_on_date(uid, on_date, source)
                    .collapse()?;
                Ok(ret)
            },
            FindScheduleForActivation { uid, start_date, source, stp_indicator } => {
                let ret = self.find_schedule_for_activation(uid, stp_indicator, start_date, source)
                    .collapse()?;
                Ok(ret)
            },
            RequestScheduleDetails(uu) => {
                let ret = self.request_schedule_details(uu)
                    .collapse()?;
                Ok(ret)
            },
            Ping => {
                let ret = format!("Hi from {}!", env!("CARGO_PKG_VERSION"));
                let ret = Ok(ret).try_into()?;
                Ok(ret)
            },
            QueueUpdateJob(jt) => {
                let ret = self.dls_tx.send(jt)
                    .map_err(|e| FahrplanError::InternalError(e.to_string()))
                    .try_into()?;
                Ok(ret)
            }
        }
    }
}
