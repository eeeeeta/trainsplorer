//! Worker threads, responsible for dealing with a stream of messages.

use chashmap::CHashMap;
use crossbeam_channel::Receiver;
use std::sync::Arc;
use tspl_sqlite::uuid::Uuid;
use tspl_util::rpc::MicroserviceRpc;
use tspl_util::user_agent;
use ntrod_types::movements::{Records, MvtBody, self};
use tspl_zugfuhrer::types::{Train, TrainMvt};
use tspl_fahrplan::types::Schedule;
use reqwest::header::HeaderMap;
use reqwest::Method;
use failure::format_err;
use chrono::prelude::*;
use log::*;

use crate::errors::*;

pub type TrustTsplStore = Arc<CHashMap<String, Uuid>>;

pub enum WorkerMessage {
    Movement(String)
}
pub struct NrodWorker {
    rx: Receiver<WorkerMessage>,
    /// A map of TRUST IDs to `tspl-zugfuhrer` UUIDs.
    trust_to_tspl: TrustTsplStore,
    /// RPC for `tspl-zugfuhrer`.
    zrpc: MicroserviceRpc,
}
// The last two digits of a TRUST ID signify the day of the month
// when the train first set off.
fn trust_id_extract_day_of_month(tid: &str) -> Result<u32> {
    let last_two = tid.get(8..10)
        .ok_or(format_err!("TRUST ID isn't 10 characters"))?;
    let as_dom = last_two.parse()?;
    Ok(as_dom)
}
impl NrodWorker {
    pub fn new(rx: Receiver<WorkerMessage>, ts: TrustTsplStore, base_url: String) -> Self {
        let zrpc = MicroserviceRpc::new(user_agent!(), "zugfuhrer", base_url);
        Self { rx, trust_to_tspl: ts, zrpc }
    }
    fn lookup_trust_id(&mut self, tid: &str, date: NaiveDate) -> Result<Uuid> {
        if let Some(ret) = self.trust_to_tspl.get(tid) {
            return Ok(*ret);
        }
        debug!("Querying zugfuhrer for TRUST ID {}", tid);
        let train: Train = self.zrpc.req(Method::GET, format!("/trains/by-trust-id/{}/{}", tid, date))?;
        self.trust_to_tspl.insert(tid.to_owned(), train.tspl_id);
        Ok(train.tspl_id)
    }
    fn handle_activation(&mut self, a: movements::Activation) -> Result<()> {
        use self::movements::ScheduleSource;

        debug!("Processing activation of train {}...", a.train_id);
        let src = match a.schedule_source {
            ScheduleSource::CifItps => Schedule::SOURCE_ITPS,
            ScheduleSource::VstpTops => Schedule::SOURCE_VSTP,
        };
        let mut hdrs = HeaderMap::new();
        hdrs.insert("X-tspl-schedule-uid", a.train_uid.parse()?);
        hdrs.insert("X-tspl-schedule-start-date", a.schedule_start_date.to_string().parse()?);
        hdrs.insert("X-tspl-schedule-source", src.to_string().parse()?);
        hdrs.insert("X-tspl-schedule-stp-indicator", a.schedule_type.as_char().to_string().parse()?);
        hdrs.insert("X-tspl-activation-date", a.origin_dep_timestamp.date().to_string().parse()?);
        let train: Train = self.zrpc.req_with_headers(Method::POST, "/trains/activate", hdrs)?;
        debug!("Activated as {}; linking TRUST ID...", train.tspl_id);
        let _: () = self.zrpc.req(Method::POST, format!("/trains/{}/trust-id/{}", train.tspl_id, a.train_id))?;
        info!("Activated train {} as {}.", a.train_id, train.tspl_id);
        self.trust_to_tspl.insert(a.train_id, train.tspl_id);
        Ok(())
    }
    fn handle_cancellation(&mut self, c: movements::Cancellation) -> Result<()> {
        debug!("Processing cancellation of train {}...", c.train_id);
        let tspl_id = self.lookup_trust_id(&c.train_id, c.canx_timestamp.date())?;
        let _: () = self.zrpc.req(Method::POST, format!("/trains/{}/cancel", tspl_id))?;
        info!("Train {} ({}) cancelled.", c.train_id, tspl_id);
        Ok(())
    }
    fn handle_movement(&mut self, m: movements::Movement) -> Result<()> {
        use self::movements::EventType;

        debug!("Processing movement of train {} at STANOX {}...", m.train_id, m.loc_stanox);
        if m.offroute_ind {
            debug!("Train {} off route.", m.train_id);
            return Ok(());
        }
        let planned_ts = match m.planned_timestamp {
            Some(t) => t,
            None => {
                Err(format_err!("Movement has no planned timestamp, cannot continue"))?
            }
        };
        let tspl_id = self.lookup_trust_id(&m.train_id, planned_ts.date())?;
        let mut hdrs = HeaderMap::new();
        hdrs.insert("X-tspl-mvt-stanox", m.loc_stanox.parse()?);
        hdrs.insert("X-tspl-mvt-planned-time", planned_ts.time().to_string().parse()?);
        let start_dom = trust_id_extract_day_of_month(&m.train_id)?;
        // If the day of the month is equal to the day of month from the TRUST ID
        // (i.e. day of month at origination time), day_offset should be 0
        // (i.e. movement happens on the same day as origination). Otherwise,
        // assuming trains don't span more than one day, day_offset is 1 (on
        // the next day).
        let day_offset = if planned_ts.day() == start_dom { 0 } else { 1 };
        hdrs.insert("X-tspl-mvt-planned-day-offset", day_offset.to_string().parse()?);
        let action = match m.event_type {
            EventType::Arrival => 0,
            EventType::Destination => 0,
            EventType::Departure => 1
        };
        hdrs.insert("X-tspl-mvt-planned-action", action.to_string().parse()?);
        hdrs.insert("X-tspl-mvt-actual-time", m.actual_timestamp.time().to_string().parse()?);
        if let Some(pfm) = m.platform {
            hdrs.insert("X-tspl-mvt-platform", pfm.parse()?);
        }
        let tmvt: TrainMvt = self.zrpc.req_with_headers(Method::POST, format!("/trains/{}/trust-movement", tspl_id), hdrs)?;
        info!("Processed movement of {} ({}) at {} past {}.", m.train_id, tspl_id, m.actual_timestamp.time(), tmvt.tiploc);
        if m.train_terminated {
            let _: () = self.zrpc.req(Method::POST, format!("/trains/{}/terminate", tspl_id))?;
            info!("Train {} ({}) has terminated.", m.train_id, tspl_id);
        }
        Ok(())
    }
    fn on_mvt_message(&mut self, st: &str) {
        use self::MvtBody::*;
        info!("Processing movement message batch");
        let recs: Records = match serde_json::from_str(st) {
            Ok(r) => r,
            Err(e) => {
                warn!("Failed to parse movement records: {}", e);
                warn!("Records were: {}", st);
                return;
            }
        };
        for rec in recs {
            let ret: Result<()> = match rec.body {
                Activation(r) => self.handle_activation(r),
                Movement(m) => self.handle_movement(m),
                Cancellation(c) => self.handle_cancellation(c),
                _ => Err(format_err!("message type not yet implemented"))
            };
            if let Err(e) = ret {
                warn!("Error processing (type {}): {}", rec.header.msg_type, e);
            }
        }
    }
    pub fn run(&mut self) {
        loop {
            let data = self.rx.recv().unwrap();
            match data {
                WorkerMessage::Movement(d) => self.on_mvt_message(&d)
            }
        }
    }
}
