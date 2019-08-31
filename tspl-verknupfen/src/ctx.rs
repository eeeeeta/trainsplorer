//! Main app context.

use rouille::{Request, Response, router};
use chrono::prelude::*;
use tspl_util::rpc::MicroserviceRpc;
use tspl_util::user_agent;
use tspl_util::http::HttpServer;
use tspl_zugfuhrer::types::TrainMvt;
use std::collections::HashSet;
use reqwest::Method;
use chrono::Duration;
use log::*;

use crate::config::Config;
use crate::types::*;
use crate::errors::*;

pub struct App {
    /// RPC for `tspl-zugfuhrer`.
    zrpc: MicroserviceRpc,
    /// RPC for `tspl-fahrplan`.
    frpc: MicroserviceRpc,
}

impl HttpServer for App {
    type Error = VerknupfenError;

    fn on_request(&self, req: &Request) -> VerknupfenResult<Response> {
        router!(req,
            (GET) (/) => {
                Ok(Response::text(user_agent!()))
            },
            (GET) (/train-movements/through/{tiploc}/at/{ts: NaiveDateTime}/within-secs/{dur: u32}) => {
                self.get_mvts_passing_through(tiploc, ts, Duration::seconds(dur as _))
                    .map(|x| Response::json(&x))
            },
            _ => {
                Err(VerknupfenError::InvalidPath)
            }
        )
    }
}

impl App {
    pub fn new(cfg: &Config) -> Self {
        let zrpc = MicroserviceRpc::new(user_agent!(), "zugfuhrer", cfg.service_zugfuhrer.clone());
        let frpc = MicroserviceRpc::new(user_agent!(), "fahrplan", cfg.service_fahrplan.clone());
        Self { zrpc, frpc }
    } 
    fn get_mvts_passing_through(&self, tpl: String, ts: NaiveDateTime, within_dur: Duration) -> VerknupfenResult<MvtQueryResponse> {
        use tspl_fahrplan::types::MvtQueryResponse as FahrplanMQR;
        use tspl_zugfuhrer::types::MvtQueryResponse as ZugfuhrerMQR;

        // Request movements from the two microservices.
        let furl = format!("/schedule-movements/through/{}/at/{}/within-secs/{}", tpl, ts.format("%+"), within_dur.num_seconds()); 
        let mut fahrplan: FahrplanMQR = self.frpc.req(Method::GET, furl)?;
        let zurl = format!("/train-movements/through/{}/at/{}/within-secs/{}", tpl, ts.format("%+"), within_dur.num_seconds()); 
        let zugfuhrer: ZugfuhrerMQR = self.zrpc.req(Method::GET, zurl)?;

        // Look through the tspl-zugfuhrer trains, and remove any schedules
        // from the tspl-fahrplan output which have a corresponding train.
        // (Since movements get copied, we'll get the information from the train
        // movements.)
        //
        // This is done by building a HashSet of (uid, start_date, stp_indicator, source)
        // and removing schedules which match this set.
        let mut scheds = HashSet::new();
        for (_, train) in zugfuhrer.trains.iter() {
            scheds.insert((&train.parent_uid, train.parent_start_date, &train.parent_stp_indicator, train.parent_source));
        }
        fahrplan.schedules.retain(|_, sched| {
            !scheds.contains(&(&sched.uid, sched.start_date, &sched.stp_indicator, sched.source as _))
        });

        let mut dmvts = vec![];

        // Now, process all of the train movements, deduplicating information to create
        // `DeduplicatedMvt` objects.
        for (tid, tmvts) in zugfuhrer.mvts {
            if tmvts.len() == 0 {
                warn!("Train #{} has no associated movements", tid);
                continue;
            }
            let train = zugfuhrer.trains.get(&tid)
                .ok_or(VerknupfenError::RemoteInvariantsViolated)?;
            let mut time_scheduled = None;
            let mut time = None;
            let mut pfm_scheduled = None;
            let mut pfm_actual = None;
            let mut pfm_suppr = false;
            let mut actual = false;
            let mut tiploc = None;
            let mut action = None;
            for mvt in tmvts {
                tiploc = Some(mvt.tiploc);
                action = Some(mvt.action);
                match mvt.source {
                    TrainMvt::SOURCE_SCHED_ITPS => {
                        pfm_scheduled = mvt.platform;
                        time_scheduled = Some(TimeWithSource {
                            time: mvt.time,
                            source: mvt.source
                        });
                    },
                    TrainMvt::SOURCE_TRUST => {
                        actual = true;
                        time = Some(TimeWithSource {
                            time: mvt.time,
                            source: mvt.source
                        });
                        pfm_actual = mvt.platform;
                    },
                    TrainMvt::SOURCE_DARWIN => {
                        pfm_actual = pfm_actual.or(mvt.platform);
                        if !actual {
                            time = Some(TimeWithSource {
                                time: mvt.time,
                                source: mvt.source
                            });
                        }
                    },
                    mvts => {
                        warn!("Currently not processing mvts of source {}", mvts);
                    }
                }
                // If even one of the tmvts demands suppression, we should
                // probably suppress platform display.
                if mvt.pfm_suppr {
                    pfm_suppr = true;
                }
            }
            let (time, time_scheduled) = if time.is_none() {
                assert_eq!(actual, false);
                (time_scheduled.unwrap(), None)
            }
            else {
                (time.unwrap(), time_scheduled)
            };
            let dmvt = DeduplicatedMvt {
                src: DeduplicatedMvtSource::Train(tid),
                tiploc: tiploc.unwrap(),
                action: action.unwrap(),
                canx: train.cancelled,
                time, actual, time_scheduled,
                pfm_scheduled, pfm_actual, pfm_suppr
            };
            dmvts.push(dmvt);
        }

        // For all remaining schedule movements, create DeduplicatedMvts
        // if their parent schedules still exist (i.e. they haven't
        // been obsoleted).

        for (sid, mvt) in fahrplan.mvts {
            let _ = match fahrplan.schedules.get(&sid) {
                Some(s) => s,
                None => continue
            };
            dmvts.push(DeduplicatedMvt {
                src: DeduplicatedMvtSource::Schedule(sid),
                tiploc: mvt.tiploc,
                action: mvt.action,
                time: TimeWithSource {
                    time: mvt.time,
                    source: TrainMvt::SOURCE_SCHED_ITPS
                },
                actual: false,
                time_scheduled: None,
                canx: false,
                pfm_scheduled: mvt.platform,
                pfm_actual: None,
                pfm_suppr: false
            })
        }

        Ok(MvtQueryResponse {
            mvts: dmvts,
            schedules: fahrplan.schedules,
            trains: zugfuhrer.trains
        })
    }
}
