//! Handles returning data from movement searches.

use tspl_util::rpc::MicroserviceRpc;
use tspl_util::user_agent;
use tspl_sqlite::TsplPool;
use tspl_sqlite::traits::*;
use tspl_verknupfen::types as vkt;
use tspl_fahrplan::types as fpt;
use tspl_zugfuhrer::types as zft;
use vkt::DeduplicatedMvtSource;
use tspl_nennen::types::StationName;
use std::collections::HashMap;
use reqwest::Method;
use rayon::prelude::*;
use chrono::prelude::*;
use chrono::Duration;
use log::*;

use crate::config::Config;
use crate::errors::*;
use crate::templates::movements::{MovementOrigDest, MovementDesc};
use crate::util;

pub struct MovementSearcher {
    pool: TsplPool,
    frpc: MicroserviceRpc,
    zrpc: MicroserviceRpc,
    vrpc: MicroserviceRpc
}
fn tiploc_nice_name(conn: &Connection, tpl: &str) -> WebResult<String> {
    let names = StationName::from_select(conn, "WHERE tiploc = ?", params![tpl])?;
    let ret = names.into_iter().nth(0)
        .map(|x| x.name)
        .unwrap_or_else(|| {
            tpl.to_owned()
        });
    Ok(ret)
}
impl MovementSearcher {
    pub fn new(pool: TsplPool, cfg: &Config) -> Self {
        let frpc = MicroserviceRpc::new(user_agent!(), "fahrplan", cfg.service_fahrplan.clone());
        let zrpc = MicroserviceRpc::new(user_agent!(), "zugfuhrer", cfg.service_zugfuhrer.clone());
        let vrpc = MicroserviceRpc::new(user_agent!(), "verknupfen", cfg.service_verknupfen.clone());
        Self { frpc, zrpc, vrpc, pool }
    }
    pub fn get_movements_through_tiploc(&self, tpl: &str, ts: NaiveDateTime, within_dur: Duration) -> WebResult<Vec<MovementDesc>> {
        let db = self.pool.get()?;
        let url = format!("/train-movements/through/{}/at/{}/within-secs/{}", tpl, ts.format("%Y-%m-%dT%H:%M:%S"), within_dur.num_seconds()); 
        let mqr: vkt::MvtQueryResponse = self.vrpc.req(Method::GET, url)?;
        let mut train_origdests = mqr.trains
            .par_iter()
            .flat_map(|(id, x)| {
                let url = format!("/train/{}", x.tspl_id);
                let tdets: zft::TrainDetails = match self.zrpc.req(Method::GET, url) {
                    Ok(d) => d,
                    Err(e) => {
                        warn!("Fetching train details for {} failed: {}", x.tspl_id, e);
                        return None;
                    }
                };
                Some((*id, MovementOrigDest {
                    orig: tdets.mvts.first().map(|x| x.tiploc.clone())
                        .unwrap_or("nowhere".into()),
                    dest: tdets.mvts.last().map(|x| x.tiploc.clone())
                        .unwrap_or("nowhere".into()),
                }))
            })
            .collect::<HashMap<i64, MovementOrigDest>>();
        let mut sched_origdests = mqr.schedules
            .par_iter()
            .flat_map(|(id, x)| {
                let url = format!("/schedule/{}", x.tspl_id);
                let sdets: fpt::ScheduleDetails = match self.frpc.req(Method::GET, url) {
                    Ok(d) => d,
                    Err(e) => {
                        warn!("Fetching schedule details for {} failed: {}", x.tspl_id, e);
                        return None;
                    }
                };
                Some((*id, MovementOrigDest {
                    orig: sdets.mvts.first().map(|x| x.tiploc.clone())
                        .unwrap_or("nowhere".into()),
                    dest: sdets.mvts.last().map(|x| x.tiploc.clone())
                        .unwrap_or("nowhere".into()),
                }))
            })
            .collect::<HashMap<i64, MovementOrigDest>>();
        let mut ret: Vec<MovementDesc> = Vec::with_capacity(mqr.mvts.len());
        for (_, odest) in train_origdests.iter_mut() {
            odest.orig = tiploc_nice_name(&db, &odest.orig)?;
            odest.dest = tiploc_nice_name(&db, &odest.dest)?;
        }
        for (_, odest) in sched_origdests.iter_mut() {
            odest.orig = tiploc_nice_name(&db, &odest.orig)?;
            odest.dest = tiploc_nice_name(&db, &odest.dest)?;
        }
        for mvt in mqr.mvts {
            let (ps, pt, odest, canx) = match mvt.src {
                DeduplicatedMvtSource::Schedule(s) => {
                    let psu = mqr.schedules.get(&s)
                        .ok_or(WebError::RemoteInvariantsViolated)?
                        .tspl_id;
                    let odest = sched_origdests.get(&s)
                        .ok_or(WebError::DetailsFetchFailed)?;
                    (Some(psu), None, odest.clone(), false)
                },
                DeduplicatedMvtSource::Train(t) => {
                    let train = mqr.trains.get(&t)
                        .ok_or(WebError::RemoteInvariantsViolated)?;
                    let odest = train_origdests.get(&t)
                        .ok_or(WebError::DetailsFetchFailed)?;
                    (None, Some(train.tspl_id), odest.clone(), train.cancelled)
                }
            };
            let pfm_changed = mvt.pfm_scheduled.is_some() && mvt.pfm_actual != mvt.pfm_scheduled;
            let delayed = if let Some(ts) = mvt.time_scheduled {
                ts.time != mvt.time.time
            }
            else {
                false
            };
            let mdesc = MovementDesc {
                parent_sched: ps.map(|x| x.to_string()),
                parent_train: pt.map(|x| x.to_string()),
                tiploc: mvt.tiploc,
                action: util::action_to_icon(mvt.action),
                time: util::format_time_with_half(&mvt.time.time),
                time_scheduled: mvt.time_scheduled
                    .map(|x| util::format_time_with_half(&x.time)),
                actual: mvt.actual,
                platform: mvt.pfm_actual,
                pfm_changed,
                pfm_suppr: mvt.pfm_suppr,
                action_past_tense: util::action_past_tense(mvt.action),
                delayed: delayed,
                canx,
                orig_dest: odest,
                _time: mvt.time.time,
                _action: mvt.action
            };
            ret.push(mdesc);
        }
        ret.sort_by_key(|d| (d._time, d._action));
        Ok(ret)
    }
}
