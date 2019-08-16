//! Handling Darwin messages.

use chashmap::CHashMap;
use crossbeam_channel::Receiver;
use std::sync::Arc;
use tspl_sqlite::uuid::Uuid;
use tspl_util::rpc::{RpcError, MicroserviceRpc};
use tspl_util::user_agent;
use tspl_zugfuhrer::types::{Train, TrainMvt};
use darwin_types::pport::{Pport, PportElement};
use darwin_types::forecasts::{Ts, PlatformData, TsTimeData};
use reqwest::header::HeaderMap;
use reqwest::Method;
use failure::format_err;
use chrono::prelude::*;
use log::*;

use crate::errors::*;

pub type DarwinRidStore = Arc<CHashMap<String, Uuid>>;

pub enum DarwinMessage {
    Pport(String)
}

// Scheduled day offset.
pub struct DarwinWorker {
    rx: Receiver<DarwinMessage>,
    /// A map of Darwin RIDs to `tspl-zugfuhrer` UUIDs.
    rid_to_tspl: DarwinRidStore,
    /// RPC for `tspl-zugfuhrer`.
    zrpc: MicroserviceRpc,
}
impl DarwinWorker {
    pub fn new(rx: Receiver<DarwinMessage>, ts: DarwinRidStore, base_url: String) -> Self {
        let zrpc = MicroserviceRpc::new(user_agent!(), "zugfuhrer", base_url);
        Self { rx, rid_to_tspl: ts, zrpc }
    }
    fn lookup_or_activate_train(&mut self, rid: &str, uid: &str, ssd: NaiveDate) -> Result<Uuid> {
        if let Some(ret) = self.rid_to_tspl.get(rid) {
            return Ok(*ret);
        }
        debug!("Querying zugfuhrer for RID {}", rid);
        let train: Option<Train> = match self.zrpc.req(Method::GET, format!("/trains/by-darwin-rid/{}", rid)) {
            Ok(t) => Some(t),
            Err(RpcError::RemoteNotFound) => None,
            Err(e) => Err(e)?
        };
        if let Some(train) = train {
            self.rid_to_tspl.insert(rid.to_owned(), train.tspl_id);
            Ok(train.tspl_id)
        }
        else {
            debug!("Activating Darwin train; (rid, uid, ssd) = ({}, {}, {})", rid, uid, ssd);
            let mut hdrs = HeaderMap::new();
            hdrs.insert("X-tspl-darwin-rid", rid.parse()?);
            hdrs.insert("X-tspl-schedule-uid", uid.parse()?);
            hdrs.insert("X-tspl-activation-date", ssd.to_string().parse()?);
            let train: Train = self.zrpc.req_with_headers(Method::POST, "/trains/activate-fuzzy", hdrs)?;
            info!("Activated Darwin RID {} as {}.", rid, train.tspl_id);
            self.rid_to_tspl.insert(rid.to_owned(), train.tspl_id);
            Ok(train.tspl_id)
        }
    }
    fn process_ts(&mut self, ts: Ts) -> Result<()> {
        debug!("Processing TS for RID {}", ts.rid);
        let tspl_id = self.lookup_or_activate_train(&ts.rid, &ts.uid, ts.start_date)?;
        // Step 1: flatten out ts.locations into something that looks
        // more like our TrainMvt schema.
        struct TsUpdate {
            // The scheduled time of this movement - i.e. this will
            // refer to the movement we're looking to update.
            time_sched: NaiveTime,
            // Scheduled day offset.
            day_offset: u8,
            tiploc: String,
            action: u8,
            // Data about what actual / estimated times exist.
            tstd: TsTimeData,
            platform: Option<PlatformData>,
        }
        let mut day_offset = 0;
        let mut last_time = None;
        let mut update_day_offset = |time: NaiveTime| {
            // Time difference            Interpret as
            // ---------------            ------------
            // Less than -6 hours         Crossed midnight
            // Between -6 and 0 hours     Back in time
            // Between 0 and +18 hours    Normal increasing time
            // Greater than +18 hours     Back in time and crossed midnight 
            match last_time {
                Some(last) => {
                    if time < last {
                        let dur = last.signed_duration_since(time);
                        if dur.num_hours() >= 6 {
                            day_offset += 1;
                        }
                    }
                    else {
                        let dur = time.signed_duration_since(last);
                        if dur.num_hours() >= 18 {
                            day_offset -= 1;
                        }
                    }
                },
                None => {
                    last_time = Some(time);
                }
            }
            day_offset
        };
        let mut updates = vec![];
        for loc in ts.locations {
            if let Some(arr) = loc.arr {
                let st = loc.timings.wta
                    .or(loc.timings.pta)
                    // Timings can sometimes be missing, for some strange
                    // reason?
                    //
                    // FIXME: less copypasta-y error message?
                    .ok_or(format_err!("some darwin timings missing"))?;
                let day_offset = update_day_offset(st);
                updates.push(TsUpdate {
                    time_sched: st,
                    tiploc: loc.tiploc.clone(),
                    tstd: arr,
                    action: 0,
                    day_offset,
                    platform: loc.plat.clone()
                });
            }
            if let Some(dep) = loc.dep {
                let st = loc.timings.wtd
                    .or(loc.timings.ptd)
                    .ok_or(format_err!("some darwin timings missing"))?;
                let day_offset = update_day_offset(st);
                updates.push(TsUpdate {
                    time_sched: st,
                    tiploc: loc.tiploc.clone(),
                    tstd: dep,
                    action: 1,
                    day_offset,
                    platform: loc.plat.clone()
                });
            }
            if let Some(pass) = loc.pass {
                let st = loc.timings.wtp
                    .ok_or(format_err!("some darwin timings missing"))?;
                let day_offset = update_day_offset(st);
                updates.push(TsUpdate {
                    time_sched: st,
                    tiploc: loc.tiploc.clone(),
                    tstd: pass,
                    action: 2,
                    day_offset,
                    platform: None
                });
            }
        }
        // Step 2: use the generated updates to fire off some RPC calls.
        for upd in updates {
            // The time associated with this movement.
            // If none of these fields are populated,
            // we don't actually have one!
            let time = upd.tstd.at
                .or(upd.tstd.wet)
                .or(upd.tstd.et);
            let time = match time {
                Some(t) => t,
                None => continue
            };
            let mut hdrs = HeaderMap::new();
            hdrs.insert("X-tspl-mvt-tiploc", upd.tiploc.parse()?);
            hdrs.insert("X-tspl-mvt-planned-time", upd.time_sched.to_string().parse()?);
            hdrs.insert("X-tspl-mvt-planned-day-offset", upd.day_offset.to_string().parse()?);
            hdrs.insert("X-tspl-mvt-planned-action", upd.action.to_string().parse()?);
            if upd.tstd.at_removed {
                // The previous actual time has just been removed!
                let hdrs = hdrs.clone();
                if let Err(e) = self.zrpc.req_with_headers::<_, ()>(Method::POST, format!("/trains/{}/darwin/at-removed", tspl_id), hdrs) {
                    warn!("Failed to remove actual time at {} for {} ({}): {}", upd.tiploc, ts.rid, tspl_id, e);
                }
            }
            let actual = upd.tstd.at.is_some();
            let unknown_delay = upd.tstd.delayed;
            if let Some(pd) = upd.platform {
                hdrs.insert("X-tspl-mvt-platform", pd.platform.parse()?);
                hdrs.insert("X-tspl-mvt-platsup", pd.platsup.to_string().parse()?);
            }
            hdrs.insert("X-tspl-mvt-updated-time", time.to_string().parse()?);
            hdrs.insert("X-tspl-mvt-time-actual", actual.to_string().parse()?);
            hdrs.insert("X-tspl-mvt-delay-unknown", unknown_delay.to_string().parse()?);
            match self.zrpc.req_with_headers::<_, TrainMvt>(Method::POST, format!("/trains/{}/darwin/update", tspl_id), hdrs) {
                Ok(_) => {
                    info!("Processed updated time for {} ({}) of {} at {}.", ts.rid, tspl_id, time, upd.tiploc);
                },
                Err(e) => {
                    warn!("Failed to process update for {} ({}) at {}: {}", ts.rid, tspl_id, upd.tiploc, e);
                    continue;
                }
            }
        }
        Ok(())
    }
    pub fn process_pport(&mut self, pp: Pport) {
        info!("Processing Darwin push port element, version {}, timestamp {}", pp.version, pp.ts);
        match pp.inner {
            PportElement::DataResponse(dr) => {
                info!("Processing Darwin data response message, origin {:?}, source {:?}, rid {:?}", dr.update_origin, dr.request_source, dr.request_id);
                for ts in dr.train_status {
                    if let Err(e) = self.process_ts(ts) {
                        warn!("Failed to process TS: {}", e);
                    }
                }
            }
            _ => {}
        }
    }
    pub fn on_pport(&mut self, st: &str) {
        match darwin_types::parse_pport_document(st.as_bytes()) {
            Ok(pp) => self.process_pport(pp),
            Err(e) => {
                warn!("Failed to parse push port document: {}", e);
                warn!("Document was: {}", st);
            }
        }
    }
    pub fn run(&mut self) {
        loop {
            let data = self.rx.recv().unwrap();
            match data {
                DarwinMessage::Pport(d) => self.on_pport(&d)
            }
        }
    }
}

