//! Main app context.

use tspl_sqlite::TsplPool;
use rouille::{Request, Response, router};
use reqwest::Method;
use chrono::prelude::*;
use chrono::offset::Local;
use log::*;
use tspl_util::rpc::MicroserviceRpc;
use tspl_util::{user_agent, extract_headers};
use tspl_util::http::{HttpServer};
use tspl_fahrplan::types as fpt;
use tspl_sqlite::uuid::Uuid;
use tspl_sqlite::traits::*;

use crate::config::Config;
use crate::errors::*;
use crate::types::*;

pub struct App {
    rpc: MicroserviceRpc,
    pool: TsplPool,
}
impl HttpServer for App {
    type Error = ZugError;

    fn on_request(&self, req: &Request) -> ZugResult<Response> {
        router!(req,
            (GET) (/) => {
                Ok(Response::text(user_agent!()))
            },
            (GET) (/trains/by-trust-id/{trust_id}) => {
                self.get_train_for_trust_id(trust_id)
                    .map(|x| Response::json(&x))
            },
            (POST) (/trains/{tid: Uuid}/trust-id/{trust_id}) => {
                self.associate_trust_id(tid, trust_id)
                    .map(|x| Response::json(&x))
            },
            (POST) (/trains/activate) => {
                extract_headers!(req, ZugError::HeadersMissing,
                                 let uid: String => "schedule-uid",
                                 let start_date: NaiveDate => "schedule-start-date",
                                 let stp_indicator: String => "schedule-stp-indicator",
                                 let source: i32 => "schedule-source",
                                 let date: NaiveDate => "activation-date");
                self.activate_train(uid, start_date, stp_indicator, source, date)
                    .map(|x| Response::json(&x))
            },
            (POST) (/trains/{tid: Uuid}/trust-movement) => {
                extract_headers!(req, ZugError::HeadersMissing,
                                 let stanox: String => "mvt-stanox",
                                 let planned_time: NaiveTime => "mvt-planned-time",
                                 let planned_day_offset: u8 => "mvt-planned-day-offset",
                                 let planned_action: u8 => "mvt-planned-action",
                                 let actual_time: NaiveTime => "mvt-actual-time",
                                 opt actual_public_time: NaiveTime => "mvt-actual-public-time",
                                 opt platform: String => "mvt-platform");
                self.process_trust_mvt_update(tid, TrustMvtUpdate {
                    stanox, planned_time, planned_day_offset,
                    planned_action, actual_time, actual_public_time,
                    platform
                })
                    .map(|x| Response::json(&x))
            },
            _ => {
                Err(ZugError::NotFound)
            }
        )
    }
}
impl App {
    pub fn new(pool: TsplPool, cfg: &Config) -> Self  {
        let rpc = MicroserviceRpc::new(user_agent!(), "fahrplan", cfg.service_fahrplan.clone());
        Self { rpc, pool }
    }
    fn process_trust_mvt_update(&self, tid: Uuid, upd: TrustMvtUpdate) -> ZugResult<TrainMvt> {
        let mut db = self.pool.get()?;
        let trans = db.transaction()?;

        let train = Train::from_select(&trans, "WHERE tspl_id = ?", params![tid])?
            .into_iter().nth(0).ok_or(ZugError::NotFound)?;
        info!("Processing {} movement at STANOX {} for train {}", upd.actual_time, upd.stanox, tid);
        
        // Get movements that match up with the provided information.
        let tmvts = TrainMvt::from_select(&trans, "WHERE parent_train = ?
                        AND time = ?
                        AND action = ?
                        AND day_offset = ?
                        AND updates IS NULL
                        AND source = ?
                        AND EXISTS(
                            SELECT * FROM corpus_entries
                            WHERE corpus_entries.tiploc = train_movements.tiploc
                            AND corpus_entries.stanox = $1
                        )",
                        params![train.id, upd.planned_time, upd.planned_action,
                        upd.planned_day_offset, TrainMvt::SOURCE_SCHED_ITPS, upd.stanox])?;
        if tmvts.len() > 1 {
            error!("Movement is ambiguous!");
            Err(ZugError::MovementsAmbiguous)?
        }
        let updates = tmvts.into_iter().nth(0).ok_or(ZugError::NotFound)?;
        let mut tmvt = TrainMvt {
            id: -1,
            parent_train: train.id,
            updates: Some(updates.id),
            tiploc: updates.tiploc,
            action: upd.planned_action,
            actual: true,
            time: upd.actual_time,
            public_time: upd.actual_public_time,
            day_offset: upd.planned_day_offset,
            source: TrainMvt::SOURCE_TRUST,
            platform: upd.platform,
            pfm_suppr: false,
            unknown_delay: false
        };
        tmvt.id = tmvt.insert_self(&trans)?;
        info!("Inserted new movement #{}", tmvt.id);
        trans.commit()?;
        Ok(tmvt)
    }
    fn get_train_for_trust_id(&self, trust_id: String) -> ZugResult<Train> {
        let db = self.pool.get()?;
        let date = Local::now().naive_local().date();
        let train = Train::from_select(&db, "WHERE trust_id = ? AND date = ?",
                                       params![trust_id, date])?
            .into_iter().nth(0).ok_or(ZugError::NotFound)?;
        Ok(train)
    }
    fn associate_trust_id(&self, tid: Uuid, trust_id: String) -> ZugResult<()> {
        let db = self.pool.get()?;
        info!("Associating {} with TRUST ID {}", tid, trust_id);
        let rows = db.execute("UPDATE trains SET trust_id = ? WHERE tspl_id = ?",
                              params![trust_id, tid])?;
        if rows == 0 {
            Err(ZugError::NotFound)?
        }
        Ok(())
    }
    fn activate_train(&self, uid: String, start_date: NaiveDate, stp_indicator: String, source: i32, date: NaiveDate) -> ZugResult<Train> {
        let mut db = self.pool.get()?;
        let trans = db.transaction()?;
        info!("Activating a train; (uid, start, stp, source, date) = ({}, {}, {}, {}, {})", uid, start_date, stp_indicator, source, date);
        // Ask tspl-fahrplan for a schedule.
        let uri = format!("/schedules/for-activation/{}/{}/{}/{}", uid, start_date, stp_indicator, source);
        let sched: Option<fpt::Schedule> = self.rpc.req(Method::GET, uri).optional()?;
        let mut train = Train {
            id: -1,
            tspl_id: Uuid::new_v4(),
            parent_uid: uid,
            parent_start_date: start_date,
            parent_stp_indicator: stp_indicator,
            date,
            trust_id: None,
            darwin_rid: None,
            headcode: None,
            crosses_midnight: false,
            parent_source: source,
            terminated: false,
            cancelled: false,
            activated: false
        };
        if let Some(sched) = sched {
            train.activated = true;
            train.crosses_midnight = sched.crosses_midnight;
            train.id = train.insert_self(&trans)?;
            // Get the actual schedule details.
            let uri = format!("/schedules/{}", sched.tspl_id);
            let details: fpt::ScheduleDetails = self.rpc.req(Method::GET, uri)?;
            // Convert each movement into a TrainMvt.
            let tmvts = details.mvts
                .into_iter()
                .map(|x| TrainMvt::from_itps(train.id, x));
            for mvt in tmvts {
                mvt.insert_self(&trans)?;
            }
            trans.commit()?;
            info!("Activated train {}", train.tspl_id);
            Ok(train)
        }
        else {
            // We didn't get a schedule, but, to avoid simply losing information,
            // make a train object anyway.
            train.id = train.insert_self(&trans)?;
            trans.commit()?;
            warn!("No schedule found for activation of train {}", train.tspl_id);
            Ok(train)
        }
    }
}
