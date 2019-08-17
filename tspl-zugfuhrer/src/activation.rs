//! Handling train activation (because it's so painful, it deserves its own module!)

use tspl_sqlite::TsplPool;
use tspl_sqlite::rusqlite::TransactionBehavior;
use tspl_sqlite::traits::*;
use tspl_fahrplan::types as fpt;
use tspl_util::rpc::MicroserviceRpc;
use reqwest::Method;
use chrono::prelude::*;
use log::*;

use crate::errors::*;
use crate::types::*;

pub struct Activator {
    pool: TsplPool,
    rpc: MicroserviceRpc
}
pub struct ActivationDetails {
    pub uid: String, 
    pub start_date: NaiveDate, 
    pub stp_indicator: String, 
    pub source: i32,
    pub run_date: NaiveDate
}
impl Activator {
    pub fn new(rpc: MicroserviceRpc, pool: TsplPool) -> Self {
        Self { pool, rpc }
    }
    /// Activate a train, no matter which system caused the activation to occur.
    /// If some `mvts` are provided, and the train is in need of them (`activated` flag
    /// set to false), copy those in as well. (If this request races with another that
    /// sets the `activated` flag, no copying will occur.)
    ///
    /// This function is idempotent and atomic.
    fn activate_lowlevel(&self, ad: ActivationDetails, mvts: Vec<TrainMvt>) -> ZugResult<Train> {
        let mut db = self.pool.get()?;
        // Avoid race conditions by acquiring an EXCLUSIVE lock.
        let trans = db.transaction_with_behavior(TransactionBehavior::Exclusive)?;
        info!("Low-level activation; (uid, start, stp, source, run) = ({}, {}, {}, {}, {})",
              ad.uid, ad.start_date, ad.stp_indicator, ad.source, ad.run_date);
        // Inside the lock, check for a train that already matches the criteria.
        let possibles = Train::from_select(&trans, "WHERE parent_uid = ? AND parent_start_date = ?
                                                      AND parent_stp_indicator = ? AND parent_source = ?
                                                      AND date = ?",
                                                    params![ad.uid, ad.start_date, ad.stp_indicator, ad.source, ad.run_date])?;
        let mut train = None;
        if possibles.len() > 0 {
            let ret = possibles.into_iter().nth(0).unwrap(); 
            if !ret.activated && mvts.len() > 0 {
                info!("Already found a train {}, but it's unactivated; copying...", ret.tspl_id);
                train = Some(ret);
            }
            else {
                return Ok(ret);
            }
        }
        // Otherwise, activate the train by doing a lot of copying.
        let mut train = match train {
            Some(t) => t,
            None => {
                let mut t = Train {
                    id: -1,
                    tspl_id: Uuid::new_v4(),
                    parent_uid: ad.uid,
                    parent_start_date: ad.start_date,
                    parent_stp_indicator: ad.stp_indicator,
                    date: ad.run_date,
                    trust_id: None,
                    darwin_rid: None,
                    headcode: None,
                    crosses_midnight: false,
                    parent_source: ad.source,
                    terminated: false,
                    cancelled: false,
                    activated: false
                };
                t.id = t.insert_self(&trans)?;
                t
            }
        };
        if !train.activated && mvts.len() > 0 {
            // Copy the movements, if necessary.
            for mut mvt in mvts {
                mvt.parent_train = train.id;
                mvt.insert_self(&trans)?;
            }
            train.activated = true;
            trans.execute("UPDATE trains SET activated = true WHERE id = ?", params![train.id])?;
        }
        trans.commit()?;
        info!("Activated train {} (flag = {}).", train.tspl_id, train.activated);
        Ok(train)
    }
    pub fn get_tmvts_for_schedule(&self, sched: &fpt::Schedule) -> ZugResult<Vec<TrainMvt>> {
        // Get the actual schedule details.
        let uri = format!("/schedule/{}", sched.tspl_id);
        let details: fpt::ScheduleDetails = self.rpc.req(Method::GET, uri)?;
        // Convert each movement into a TrainMvt.
        let tmvts = details.mvts
            .into_iter()
            .map(|x| TrainMvt::from_itps(x))
            .collect();
        Ok(tmvts)
    }
    pub fn activate_train_darwin(&self, uid: String, run_date: NaiveDate) -> ZugResult<Train> {
        info!("Activating a train from Darwin; (uid, date) = ({}, {})", uid, run_date);
        // Ask tspl-fahrplan for the authoritative schedule on that date.
        let uri = format!("/schedules/by-uid-on-date/{}/{}/{}", uid, run_date, fpt::Schedule::SOURCE_ITPS);
        // Not optional here - if we don't find a schedule, we can't actually do the activation!
        let sched: fpt::Schedule = self.rpc.req(Method::GET, uri)?;
        let tmvts = self.get_tmvts_for_schedule(&sched)?;
        // Do it!
        let ret = self.activate_lowlevel(ActivationDetails { 
            uid,
            start_date: sched.start_date,
            stp_indicator: sched.stp_indicator,
            source: sched.source as _,
            run_date 
        }, tmvts)?;
        Ok(ret)
    }
    pub fn activate_train_nrod(&self, uid: String, start_date: NaiveDate, stp_indicator: String, source: i32, date: NaiveDate) -> ZugResult<Train> {
        info!("Activating a train from NROD; (uid, start, stp, source, date) = ({}, {}, {}, {}, {})", uid, start_date, stp_indicator, source, date);
        // Ask tspl-fahrplan for a schedule.
        let uri = format!("/schedules/for-activation/{}/{}/{}/{}", uid, start_date, stp_indicator, source);
        let sched: Option<fpt::Schedule> = self.rpc.req(Method::GET, uri).optional()?;
        let mvts = if let Some(sched) = sched {
            self.get_tmvts_for_schedule(&sched)?
        }
        else {
            warn!("No ITPS schedule found; (uid, start, stp, source, date) = ({}, {}, {}, {}, {})", uid, start_date, stp_indicator, source, date);
            vec![]
        };
        let ret = self.activate_lowlevel(ActivationDetails { uid, start_date, stp_indicator, source, run_date: date }, mvts)?;
        Ok(ret)
    }
}
