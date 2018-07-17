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
    let trans = conn.transaction()?;
    debug!("Processing Darwin push port element, version {}, timestamp {}", pp.version, pp.ts);
    match pp.inner {
        PportElement::DataResponse(dr) => {
            debug!("Processing Darwin data response message, origin {:?}, source {:?}, rid {:?}", dr.update_origin, dr.request_source, dr.request_id);
            for ts in dr.train_status {
                worker.incr("darwin.ts_recv");
                match process_ts(&trans, ts) {
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
    trans.commit()?;
    Ok(())
}
pub fn activate_train_from_darwin<T: GenericConnection>(conn: &T, rid: String, uid: String, start_date: NaiveDate) -> Result<Train> {
    debug!("Activating Darwin train with RID {}, UID {}, start_date {}", rid, uid, start_date);
    let scheds = Schedule::from_select(conn, "WHERE uid = $1 AND start_date <= $2 AND end_date >= $2", &[&uid, &start_date])?;
    debug!("{} potential schedules", scheds.len());
    let mut auth_schedule: Option<Schedule> = None;
    for sched in scheds {
        if !sched.is_authoritative(conn, start_date)? {
            debug!("Schedule #{} is superseded.", sched.id);
        }
        else {
            if auth_schedule.is_some() {
                return Err(NrodError::TwoAuthoritativeSchedulesDarwin(sched.id, auth_schedule.as_ref().unwrap().id));
            }
            auth_schedule = Some(sched);
        }
    }
    let auth_schedule = if let Some(sch) = auth_schedule {
        sch
    }
    else {
        return Err(NrodError::NoAuthoritativeSchedulesDarwin {
            rid, uid, start_date
        });
    };
    let mut train = Train {
        id: -1,
        parent_sched: auth_schedule.id,
        trust_id: None,
        date: start_date,
        signalling_id: None,
        cancelled: false,
        terminated: false,
        nre_id: Some(rid)
    };
    let id = train.insert_self(conn)?;
    debug!("Inserted train as #{}", id);
    train.id = id;
    Ok(train)
}
pub fn get_train_for_rid_uid_ssd<T: GenericConnection>(conn: &T, rid: String, uid: String, start_date: NaiveDate) -> Result<Train> {
    let rid_trains = Train::from_select(conn, "WHERE nre_id = $1", &[&rid])?;
    if let Some(t) = rid_trains.into_iter().nth(0) {
        debug!("Found pre-linked train {} (TRUST id {:?}) for Darwin RID {}", t.id, t.trust_id, rid);
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
            debug!("Linked RID {} to train {} (TRUST ID {:?})", rid, t.id, t.trust_id);
            Ok(t)
        },
        None => {
            debug!("Link failed; activating Darwin train...");
            Ok(activate_train_from_darwin(conn, rid, uid, start_date)?)
        }
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
        if tstd.at_removed {
            // TODO: make this source check less brittle
            conn.execute("DELETE FROM train_movements WHERE parent_mvt = $1 AND source = 1", &[&mvt.id])?;
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
