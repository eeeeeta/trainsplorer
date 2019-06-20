//! Main context for requests / responses and that

use tspl_sqlite::TsplPool;
use tspl_sqlite::traits::*;
use crate::dl_scheduler::DlSender;
use crate::proto::{FahrplanRequest, ScheduleDetails, FahrplanResponse, FahrplanError};
use crate::types::{Schedule, ScheduleMvt};
use crate::errors::Result;
use std::convert::TryInto;

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
    pub fn process_request(&mut self, req: FahrplanRequest) -> Result<FahrplanResponse<'static>> {
        use self::FahrplanRequest::*;
        match req {
            FindSchedulesWithUid { uid } => {
                let ret = self.find_schedules_with_uid(uid)
                    .map_err(|e| FahrplanError::InternalError(e.to_string()))
                    .try_into()?;
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
            },
            _ => {
                let res: std::result::Result<(), _> = Err(FahrplanError::InternalError("not implemented".into()));
                let ret = res
                    .try_into()?;
                Ok(ret)
            }
        }
    }
}
