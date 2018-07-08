use errors::NrodError;
use osms_db::ntrod::types::*;
use osms_db::db::{DbType, InsertableDbType, GenericConnection};
use darwin_types::pport::{Pport, PportElement};
use darwin_types::forecasts::Ts;
use super::NtrodWorker;
use chrono::NaiveDate;

type Result<T> = ::std::result::Result<T, NrodError>;

pub fn process_darwin_pport(worker: &mut NtrodWorker, pp: Pport) -> Result<()> {
    let conn = worker.pool.get().unwrap();
    debug!("Processing Darwin push port element, version {}, timestamp {}", pp.version, pp.ts);
    match pp.inner {
        PportElement::DataResponse(dr) => {
            debug!("Processing Darwin data response message, origin {:?}, source {:?}, rid {:?}", dr.update_origin, dr.request_source, dr.request_id);
            for ts in dr.train_status {
                worker.incr("darwin.ts_recv");
                match process_ts(&*conn, ts) {
                    Ok(_) => worker.incr("darwin.ts_processed"),
                    Err(e) => {
                        worker.incr("darwin.ts_fail");
                        error!("Failed to process TS: {}", e);
                    }
                }
            }
        },
        _ => {
            worker.incr("darwin.unknown_message");
            return Err(NrodError::UnimplementedMessageType("darwin_unknown".into()));
        }
    }
    Ok(())
}
pub fn get_train_for_rid_uid_ssd<T: GenericConnection>(conn: &T, rid: String, uid: String, start_date: NaiveDate) -> Result<Train> {
    let rid_trains = Train::from_select(conn, "WHERE nre_id = $1", &[&rid])?;
    if let Some(t) = rid_trains.into_iter().nth(0) {
        debug!("Found pre-linked train {} (TRUST id {}) for Darwin RID {}", t.id, t.trust_id, rid);
        return Ok(t);
    }
    debug!("Trying to link RID {} (uid {}, start_date {}) to a train...", rid, uid, start_date);
    let trains = Train::from_select(conn, "WHERE EXISTS(SELECT * FROM schedules WHERE uid = $1 AND id = trains.parent_sched) AND date = $2", &[&uid, &start_date])?;
    if trains.len() > 1 {
        return Err(NrodError::AmbiguousTrains { rid, uid, start_date });
    }
    match trains.into_iter().nth(0) {
        Some(t) => {
            conn.execute("UPDATE trains SET nre_id = $1 WHERE id = $2", &[&rid, &t.id])?;
            debug!("Linked RID {} to train {} (TRUST ID {})", rid, t.id, t.trust_id);
            Ok(t)
        },
        None => Err(NrodError::RidLinkFailed { rid, uid, start_date })
    }
}
pub fn process_ts<T: GenericConnection>(conn: &T, ts: Ts) -> Result<()> {
    debug!("Processing update to rid {} (uid {}, start_date {})...", ts.rid, ts.uid, ts.start_date);
    let train = get_train_for_rid_uid_ssd(conn, ts.rid, ts.uid, ts.start_date)?;
    // vec of (tiploc, action, time, tstimedata)
    let mut updates = vec![];
    for loc in ts.locations {
        if let Some(arr) = loc.arr {
            let st = loc.timings.wta
                .or(loc.timings.pta)
                .ok_or(NrodError::DarwinTimingsMissing)?;
            updates.push((loc.tiploc.clone(), 0, st, arr));
        }
        if let Some(dep) = loc.dep {
            let st = loc.timings.wtd
                .or(loc.timings.ptd)
                .ok_or(NrodError::DarwinTimingsMissing)?;
            updates.push((loc.tiploc.clone(), 1, st, dep));
        }
        if let Some(pass) = loc.pass {
            let st = loc.timings.wtp
                .ok_or(NrodError::DarwinTimingsMissing)?;
            updates.push((loc.tiploc, 2, st, pass));
        }
    }
    let mut errs = vec![];
    for (tiploc, action, time, tstd) in updates {
        debug!("Querying for movements - parent_sched = {}, tiploc = {}, action = {}, time = {}", train.parent_sched, tiploc, action, time);
        let mvts = ScheduleMvt::from_select(conn, "WHERE parent_sched = $1 AND tiploc = $2 AND action = $3 AND time = $4", &[&train.parent_sched, &tiploc, &action, &time])?;
        let mvt = match mvts.into_iter().nth(0) {
            Some(m) => m,
            None => {
                errs.push(NrodError::NoMovementsFound(train.parent_sched, vec![action], vec![tiploc], Some(time)));
                continue;
            }
        };
        let actual = tstd.at.is_some();
        let time = tstd.at
            .or(tstd.wet)
            .or(tstd.et);
        let time = match time {
            Some(t) => t,
            None => {
                debug!("No useful time");
                continue;
            }
        };
        // TODO: make this source check less brittle
        let tmvts = TrainMvt::from_select(conn, "WHERE parent_mvt = $1 AND source = 1", &[&mvt.id])?;
        for tmvt in tmvts {
            if tmvt.estimated || tstd.at_removed {
                debug!("Deleting old train movement {}", tmvt.id);
                conn.execute("DELETE FROM train_movements WHERE id = $1", &[&tmvt.id])?;
            }
        }
        let tmvt = TrainMvt {
            id: -1,
            parent_train: train.id,
            parent_mvt: mvt.id,
            time,
            source: 1,
            estimated: !actual
        };
        let id = tmvt.insert_self(conn)?;
        debug!("Registered train movement #{}.", id);
    }
    if errs.len() > 0 {
        Err(NrodError::MultipleFailures(errs))
    }
    else {
        Ok(())
    }
}
