//! Main app context.

use tspl_sqlite::TsplPool;
use tspl_sqlite::rusqlite::TransactionBehavior;
use std::collections::HashMap;
use rouille::{Request, Response, router};
use chrono::prelude::*;
use log::*;
use tspl_util::rpc::MicroserviceRpc;
use tspl_util::{user_agent, extract_headers};
use tspl_util::http::{HttpServer};
use tspl_sqlite::uuid::Uuid;
use tspl_sqlite::traits::*;
use chrono::Duration;

use crate::config::Config;
use crate::activation::Activator;
use crate::errors::*;
use crate::types::*;

pub struct App {
    pool: TsplPool,
    activator: Activator
}
impl HttpServer for App {
    type Error = ZugError;

    fn on_request(&self, req: &Request) -> ZugResult<Response> {
        router!(req,
            (GET) (/) => {
                Ok(Response::text(user_agent!()))
            },
            (GET) (/train-movements/through/{tiploc}/at/{ts: NaiveDateTime}/within-secs/{dur: u32}) => {
                self.get_mvts_passing_through(tiploc, ts, Duration::seconds(dur as _))
                    .map(|x| Response::json(&x))
            },
            // This request URL is perhaps a bit stupid...
            (GET) (/train-movements/through/{tiploc}/and/{conn}/at/{ts: NaiveDateTime}/within-secs/{dur: u32}) => {
                self.get_connecting_mvts(tiploc, ts, Duration::seconds(dur as _), conn)
                    .map(|x| Response::json(&x))
            },
            (GET) (/trains/by-trust-id/{trust_id}/{date: NaiveDate}) => {
                self.get_train_for_trust_id(trust_id, date)
                    .map(|x| Response::json(&x))
            },
            (GET) (/trains/by-darwin-rid/{rid}) => {
                self.get_train_for_darwin_rid(rid)
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
            (POST) (/trains/activate-fuzzy) => {
                extract_headers!(req, ZugError::HeadersMissing,
                                 let uid: String => "schedule-uid",
                                 let rid: String => "darwin-rid",
                                 let date: NaiveDate => "activation-date");
                self.activate_train_darwin(uid, date, rid)
                    .map(|x| Response::json(&x))

            },
            (POST) (/trains/{tid: Uuid}/terminate) => {
                self.terminate_train(tid)
                    .map(|x| Response::json(&x))
            },
            (POST) (/trains/{tid: Uuid}/cancel) => {
                self.cancel_train(tid)
                    .map(|x| Response::json(&x))
            },
            (POST) (/trains/{tid: Uuid}/darwin/update) => {
                extract_headers!(req, ZugError::HeadersMissing,
                                 let tiploc: String => "mvt-tiploc",
                                 let planned_time: NaiveTime => "mvt-planned-time",
                                 let planned_day_offset: u8 => "mvt-planned-day-offset",
                                 let planned_action: u8 => "mvt-planned-action",
                                 let updated_time: NaiveTime => "mvt-updated-time",
                                 let time_actual: bool => "mvt-time-actual",
                                 let delay_unknown: bool => "mvt-delay-unknown",
                                 opt platsup: bool => "mvt-platsup",
                                 opt platform: String => "mvt-platform");
                self.process_darwin_mvt_update(tid, DarwinMvtUpdate {
                    tiploc, planned_time, planned_day_offset,
                    planned_action, updated_time, time_actual,
                    delay_unknown, platform,
                    platsup: platsup.unwrap_or(false)
                })
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
                Err(ZugError::InvalidPath)
            }
        )
    }
}
impl App {
    pub fn new(pool: TsplPool, cfg: &Config) -> Self  {
        let rpc = MicroserviceRpc::new(user_agent!(), "fahrplan", cfg.service_fahrplan.clone());
        let activator = Activator::new(rpc, pool.clone());
        Self { pool, activator }
    }
    // Hey, it does what it says on the tin, right?
    fn calculate_non_midnight_aware_times(ts: NaiveDateTime, within_dur: Duration) -> (NaiveTime, NaiveTime) {
        let start_ts = ts - within_dur;
        let start_time = if start_ts.date() != ts.date() {
            // Wraparound occurred, just return the start of the day (i.e. saturate)
            NaiveTime::from_hms(0, 0, 0)
        }
        else {
            start_ts.time()
        };
        let end_ts = ts + within_dur;
        let end_time = if end_ts.date() != ts.date() {
            // Same as above, but the other way this time.
            NaiveTime::from_hms(23, 59, 59)
        }
        else {
            end_ts.time()
        };
        (start_time, end_time)
    }
    // FIXME: This function doesn't yet handle trains crossing over midnight.
    // There's no reason why it couldn't in the future though; I just want to
    // move fast and break things (at the time of writing).
    fn get_mvts_passing_through(&self, tpl: String, ts: NaiveDateTime, within_dur: Duration) -> ZugResult<MvtQueryResponse> {
        let (start_time, end_time) = Self::calculate_non_midnight_aware_times(ts, within_dur);
        info!("Finding mvts passing through {} on {} between {} and {}", tpl, ts.date(), start_time, end_time);
        let db = self.pool.get()?;
        // Warning: not that heavy SQL ahead.
        let mut stmt = db.prepare("SELECT DISTINCT * 
                                              FROM train_movements AS tmvts

                                                -- Get the related trains... 
                                        INNER JOIN trains AS t 
                                                ON t.id = tmvts.parent_train
                                                -- ..and any train movements that update this one.
                                   LEFT OUTER JOIN train_movements AS updating
                                                ON updating.updates = tmvts.id

                                                -- Filter the train movements to those passing
                                                -- through the station in the given time period.
                                             WHERE (tmvts.tiploc = :tpl
                                               AND tmvts.time BETWEEN :start_time AND :end_time
                                               AND tmvts.day_offset = 0)

                                                -- Alternatively, though, the updating train movement
                                                -- could be within the time period as well, while the
                                                -- 'original' movement it updates isn't.
                                                OR (updating.tiploc = :tpl
                                               AND updating.time BETWEEN :start_time AND :end_time
                                               AND updating.day_offset = 0)

                                                -- Make sure the train movement is original (i.e.
                                                -- it doesn't update anything).
                                               AND tmvts.updates = NULL
                                               AND t.date = :date")?;
        let args = named_params! {
            ":tpl": tpl,
            ":start_time": start_time,
            ":end_time": end_time,
            ":date": ts.date()
        };
        let rows = stmt.query_map_named(args, |row| {
            Ok((
                // The original train movement, passing through `tpl`.
                TrainMvt::from_row(row, 0)?,
                // Its parent train.
                Train::from_row(row, TrainMvt::FIELDS)?,
                // An update to the original train movement, if there is one.
                TrainMvt::from_row(row, TrainMvt::FIELDS + Train::FIELDS).ok()
            ))
        })?;
        let mut tmvts: HashMap<i64, Vec<TrainMvt>> = HashMap::new();
        let mut trains = HashMap::new();
        let mut proc = 0;
        for row in rows {
            let (tmvt, train, updating_tmvt) = row?;
            proc += 1;
            trains.insert(train.id, train);
            if let Some(tmvts) = tmvts.get_mut(&tmvt.id) {
                // Entry already exists, so the original tmvt is in there.
                // We only need to add the updating one, if it exists.
                tmvts.extend(updating_tmvt);
            }
            else {
                // Entry doesn't exist; create it, and add the updating tmvt
                // as well if it exists.
                let id = tmvt.id;
                let mut ins = vec![tmvt];
                ins.extend(updating_tmvt);
                tmvts.insert(id, ins);
            }
        }
        info!("Processed {} rows for a total of {} unique tmvts and {} trains.", proc, tmvts.len(), trains.len());
        Ok(MvtQueryResponse {
            mvts: tmvts,
            trains
        })
    }
    // FIXME: As above, this function doesn't handle midnight well.
    fn get_connecting_mvts(&self, tpl: String, ts: NaiveDateTime, within_dur: Duration, connection: String) -> ZugResult<ConnectingMvtQueryResponse> {
        let (start_time, end_time) = Self::calculate_non_midnight_aware_times(ts, within_dur);
        info!("Finding mvts passing through {} and {} on {} between {} and {}", tpl, connection, ts.date(), start_time, end_time);
        let db = self.pool.get()?;
        // Warning: reasonably heavy SQL ahead.
        let mut stmt = db.prepare("SELECT DISTINCT * 
                                              FROM train_movements AS tmvts

                                                -- Get the related trains... 
                                        INNER JOIN trains AS t
                                                ON t.id = tmvts.parent_train
                                                -- and train movements that share the same
                                                -- train that might pass through the
                                                -- connecting station.
                                        INNER JOIN train_movements AS connecting
                                                ON connecting.parent_train = tmvts.parent_train

                                                -- Find updates for both sets of movements.
                                                -- (see non-connecting query)
                                   LEFT OUTER JOIN train_movements AS updating
                                                ON updating.updates = tmvts.id
                                   LEFT OUTER JOIN train_movements AS updating_connecting
                                                ON updating_connecting.updates = connecting.id

                                                -- Filter the train movements, like in
                                                -- the other query.
                                             WHERE (tmvts.tiploc = :tpl
                                               AND tmvts.time BETWEEN :start_time AND :end_time
                                               AND tmvts.day_offset = 0)

                                                OR (updating.tiploc = :tpl
                                               AND updating.time BETWEEN :start_time AND :end_time
                                               AND updating.day_offset = 0)

                                               AND tmvts.updates = NULL
                                               AND t.date = :date
                                               AND connecting.tiploc = :tpl_conn")?;
        let args = named_params! {
            ":tpl": tpl,
            ":start_time": start_time,
            ":end_time": end_time,
            ":date": ts.date(),
            ":tpl_conn": connection
        };
        let rows = stmt.query_map_named(args, |row| {
            // Golly gee, Mr. SQLite, that's a lot of columns...
            Ok((
                // The original train movement, passing through `tpl`.
                TrainMvt::from_row(row, 0)?,
                // Its parent train.
                Train::from_row(row, TrainMvt::FIELDS)?,
                // Its corresponding connecting train movement.
                TrainMvt::from_row(row, TrainMvt::FIELDS + Train::FIELDS)?,
                // An update for the original train movement, if there is one.
                TrainMvt::from_row(row, 2 * TrainMvt::FIELDS + Train::FIELDS).ok(),
                // An update for the connecting train movement, if there is one.
                TrainMvt::from_row(row, 3 * TrainMvt::FIELDS + Train::FIELDS).ok()
            ))
        })?;
        // NB: Look at the docs for `ConnectingMvtQueryResponse` to understand how
        // these structures work...
        let mut tmvts: HashMap<i64, Vec<TrainMvt>> = HashMap::new();
        let mut connecting_tmvts: HashMap<i64, Vec<TrainMvt>> = HashMap::new();
        let mut trains = HashMap::new();
        let mut proc = 0;
        for row in rows {
            let (tmvt, train, conn_tmvt, updating_tmvt, conn_updating_tmvt) = row?;
            proc += 1;
            trains.insert(train.id, train);
            let tmvt_id = tmvt.id;
            // The logic here is very similar to that used in non-connecting
            // movement queries. As such, the comments are not repeated.
            // See the other function if you're lost.
            if let Some(tmvts) = tmvts.get_mut(&tmvt_id) {
                tmvts.extend(updating_tmvt);
            }
            else {
                let mut ins = vec![tmvt];
                ins.extend(updating_tmvt);
                tmvts.insert(tmvt_id, ins);
            }
            // Same as above, but for the connecting ones.
            // This time, they key is the **corresponding (original) tmvt**'s key,
            // not the key of the connecting tmvt.
            if let Some(conn_tmvts) = connecting_tmvts.get_mut(&tmvt_id) {
                conn_tmvts.extend(conn_updating_tmvt);
            }
            else {
                let mut ins = vec![conn_tmvt];
                ins.extend(conn_updating_tmvt);
                connecting_tmvts.insert(tmvt_id, ins);
            }
        }
        info!("Processed {} rows for a total of {} unique tmvts, {} connecting, and {} trains.",
              proc, tmvts.len(), connecting_tmvts.len(), trains.len());
        Ok(ConnectingMvtQueryResponse {
            mvts: tmvts, 
            connecting_mvts: connecting_tmvts,
            trains
        })
    }
    fn process_darwin_mvt_update(&self, tid: Uuid, upd: DarwinMvtUpdate) -> ZugResult<TrainMvt> {
        let mut db = self.pool.get()?;
        let trans = db.transaction_with_behavior(TransactionBehavior::Exclusive)?;

        let train = Train::from_select(&trans, "WHERE tspl_id = ?", params![tid])?
            .into_iter().nth(0).ok_or(ZugError::NotFound)?;
        info!("Processing {} Darwin movement (actual = {}) at {} for train {}",
               upd.updated_time, upd.time_actual, upd.tiploc, tid);

        // Get movements that match up with the provided information.
        let tmvts = TrainMvt::from_select(&trans, "WHERE parent_train = ?
                        AND time = ?
                        AND action = ?
                        AND day_offset = ?
                        AND updates IS NULL
                        AND source = ?
                        AND tiploc = ?",
                        params![train.id, upd.planned_time, upd.planned_action,
                        upd.planned_day_offset, TrainMvt::SOURCE_SCHED_ITPS, upd.tiploc])?;
        if tmvts.len() > 1 {
            error!("Movement is ambiguous!");
            Err(ZugError::MovementsAmbiguous)?
        }
        let updates = tmvts.into_iter().nth(0)
            .ok_or(ZugError::MovementsNotFound)?;
        // Delete any pre-existing Darwin movements, as we're about to insert one.
        trans.execute("DELETE FROM train_movements WHERE updates = ? AND source = ?",
                      params![updates.id, TrainMvt::SOURCE_DARWIN])?;
        // Insert our updated movement.
        let mut tmvt = TrainMvt {
            id: -1,
            parent_train: train.id,
            updates: Some(updates.id),
            tiploc: upd.tiploc,
            action: upd.planned_action,
            actual: upd.time_actual,
            time: upd.updated_time,
            public_time: None,
            day_offset: upd.planned_day_offset,
            source: TrainMvt::SOURCE_DARWIN,
            platform: upd.platform,
            pfm_suppr: upd.platsup,
            unknown_delay: upd.delay_unknown
        };
        tmvt.id = tmvt.insert_self(&trans)?;
        info!("Inserted new movement #{}", tmvt.id);
        trans.commit()?;
        Ok(tmvt)
    }
    fn process_trust_mvt_update(&self, tid: Uuid, upd: TrustMvtUpdate) -> ZugResult<TrainMvt> {
        let mut db = self.pool.get()?;
        let trans = db.transaction_with_behavior(TransactionBehavior::Exclusive)?;

        let train = Train::from_select(&trans, "WHERE tspl_id = ?", params![tid])?
            .into_iter().nth(0).ok_or(ZugError::NotFound)?;
        info!("Processing {} movement at STANOX {} for train {}", upd.actual_time, upd.stanox, tid);
        // Get movements that match up with the provided information.
        let tmvts = TrainMvt::from_select(&trans, "WHERE parent_train = ?
                        AND time = ?
                        AND (action = 2 OR action = ?)
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
        let updates = tmvts.into_iter().nth(0)
            .ok_or(ZugError::MovementsNotFound)?;
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
    fn get_train_for_darwin_rid(&self, rid: String) -> ZugResult<Train> {
        let db = self.pool.get()?;
        let train = Train::from_select(&db, "WHERE darwin_rid = ?",
                                       params![rid])?
            .into_iter().nth(0).ok_or(ZugError::NotFound)?;
        Ok(train) 
    }
    fn get_train_for_trust_id(&self, trust_id: String, date: NaiveDate) -> ZugResult<Train> {
        let db = self.pool.get()?;
        let previous_date = date.pred();
        let train = Train::from_select(&db, "WHERE trust_id = ? AND (date = ? OR (crosses_midnight = true AND date = ?))",
                                       params![trust_id, date, previous_date])?
            .into_iter().nth(0).ok_or(ZugError::NotFound)?;
        Ok(train)
    }
    fn terminate_train(&self, tid: Uuid) -> ZugResult<()> {
        let db = self.pool.get()?;
        info!("Terminating train {}", tid);
        let rows = db.execute("UPDATE trains SET terminated = true WHERE tspl_id = ?",
                              params![tid])?;
        if rows == 0 {
            Err(ZugError::NotFound)?
        }
        Ok(())
    }
    fn cancel_train(&self, tid: Uuid) -> ZugResult<()> {
        let db = self.pool.get()?;
        info!("Cancelling train {}", tid);
        let rows = db.execute("UPDATE trains SET cancelled = true WHERE tspl_id = ?",
                              params![tid])?;
        if rows == 0 {
            Err(ZugError::NotFound)?
        }
        Ok(())
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
    fn activate_train_darwin(&self, uid: String, date: NaiveDate, rid: String) -> ZugResult<Train> {
        let db = self.pool.get()?;
        let mut ret = self.activator.activate_train_darwin(uid, date)?;
        db.execute("UPDATE trains SET darwin_rid = ? WHERE id = ?",
                   params![rid, ret.id])?;
        ret.darwin_rid = Some(rid);
        Ok(ret)
    }
    fn activate_train(&self, uid: String, start_date: NaiveDate, stp_indicator: String, source: i32, date: NaiveDate) -> ZugResult<Train> {
        let ret = self.activator.activate_train_nrod(uid, start_date, stp_indicator, source, date)?;
        Ok(ret)
    }
}
