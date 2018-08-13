use errors::NrodError;
use osms_db::errors::OsmsError;
use osms_db::ntrod::types::*;
use osms_db::db::{DbType, InsertableDbType, GenericConnection};
use darwin_types::pport::{Pport, PportElement};
use darwin_types::schedule::Schedule as DarwinSchedule;
use darwin_types::schedule::ScheduleLocation as DarwinScheduleLocation;
use darwin_types::schedule::{LocOr, LocOpOr, LocIp, LocOpIp, LocPp, LocDt, LocOpDt};
use darwin_types::forecasts::Ts;
use ntrod_types::schedule::Days;
use ntrod_types::cif::StpIndicator;
use std::collections::HashSet;
use super::NtrodWorker;
use chrono::{Local, NaiveDate, Duration};

type Result<T> = ::std::result::Result<T, NrodError>;

pub fn process_darwin_pport(worker: &mut NtrodWorker, pp: Pport) -> Result<()> {
    let conn = worker.pool.get().unwrap();
    let trans = conn.transaction()?;
    debug!("Processing Darwin push port element, version {}, timestamp {}", pp.version, pp.ts);
    let now = Local::now();
    if let Ok(dur) = now.signed_duration_since(pp.ts).to_std() {
        worker.latency("darwin.latency", dur);
    }
    match pp.inner {
        PportElement::DataResponse(dr) => {
            debug!("Processing Darwin data response message, origin {:?}, source {:?}, rid {:?}", dr.update_origin, dr.request_source, dr.request_id);
            for ts in dr.train_status {
                worker.incr("darwin.ts_recv");
                let now = Local::now();
                match process_ts(&trans, worker, ts) {
                    Ok(_) => worker.incr("darwin.ts_processed"),
                    Err(e) => {
                        e.send_to_stats("darwin.ts_fails", worker);
                        worker.incr("darwin.ts_fail");
                        error!("Failed to process TS: {}", e);
                    }
                }
                let after = Local::now();
                if let Ok(dur) = after.signed_duration_since(now).to_std() {
                    worker.latency("darwin.ts_process_time", dur);
                }
            }
            for sched in dr.schedule {
                worker.incr("darwin.sched.recv");
                let now = Local::now();
                match process_schedule(&trans, worker, sched) {
                    Ok(_) => worker.incr("darwin.sched.processed"),
                    Err(e) => {
                        e.send_to_stats("darwin.sched.fails", worker);
                        worker.incr("darwin.sched.fail");
                        error!("Failed to process TS: {}", e);
                    }
                }
                let after = Local::now();
                if let Ok(dur) = after.signed_duration_since(now).to_std() {
                    worker.latency("darwin.sched.process_time", dur);
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
pub fn get_train_for_rid_uid_ssd<T: GenericConnection>(conn: &T, worker: &mut NtrodWorker, rid: String, uid: String, start_date: NaiveDate) -> Result<Train> {
    let rid_trains = Train::from_select(conn, "WHERE nre_id = $1", &[&rid])?;
    if let Some(t) = rid_trains.into_iter().nth(0) {
        worker.incr("darwin.link.train_prelinked");
        debug!("Found pre-linked train {} (TRUST id {:?}) for Darwin RID {}", t.id, t.trust_id, rid);
        return Ok(t);
    }
    debug!("Trying to link RID {} (uid {}, start_date {}) to a train...", rid, uid, start_date);
    let trains = Train::from_select(conn, "WHERE EXISTS(SELECT * FROM schedules WHERE uid = $1 AND start_date <= $2 AND end_date >= $2 AND id = trains.parent_sched) AND nre_id IS NULL", &[&uid, &start_date])?;
    if trains.len() == 1 {
        let train = trains.into_iter().nth(0).unwrap();
        debug!("Found matching train #{}", train.id);
        worker.incr("darwin.link.train_matched");
        conn.execute("UPDATE trains SET nre_id = $1 WHERE id = $2", &[&rid, &train.id])?;
        return Ok(train);
    }
    else if trains.len() > 1 {
        warn!("More than one possible train for RID {} (uid {}, start_date {})", rid, uid, start_date);
    }
    let scheds = Schedule::from_select(conn, "WHERE uid = $1 AND start_date <= $2 AND end_date >= $2 AND source = 0", &[&uid, &start_date])?;
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
    let train = Train {
        id: -1,
        parent_sched: auth_schedule.id,
        trust_id: None,
        date: start_date,
        signalling_id: None,
        cancelled: false,
        terminated: false,
        nre_id: Some(rid.clone()),
        parent_nre_sched: None
    };
    let (train, was_update) = match train.insert_self(conn) {
        Ok(t) => t,
        Err(e) => {
            match e {
                OsmsError::DoubleTrainActivation(ps, date) => {
                    worker.incr("darwin.link.double_activation");
                    warn!("Train activated twice for ({}, {}), retrying...", ps, date);
                    return get_train_for_rid_uid_ssd(conn, worker, rid, uid, start_date);
                },
                e => Err(e)?
            }
        }
    };
    if was_update {
        worker.incr("darwin.link.linked_existing");
        debug!("Linked RID {} to train {} (TRUST id {:?})", rid, train.id, train.trust_id);
    }
    else {
        worker.incr("darwin.link.darwin_activation");
        debug!("Inserted train as #{}", train.id);
    }
    Ok(train)
}
pub fn process_schedule<T: GenericConnection>(conn: &T, worker: &mut NtrodWorker, sched: DarwinSchedule) -> Result<()> {
    use self::DarwinScheduleLocation::*;
    debug!("Processing Darwin schedule with rid {} (uid {}, start_date {})", sched.rid, sched.uid, sched.ssd);
    let train = get_train_for_rid_uid_ssd(conn, worker, sched.rid.clone(), sched.uid.clone(), sched.ssd.clone())?;
    let mut mvts: Vec<ScheduleMvt> = vec![];
    for loc in sched.locations {
        let tiploc;
        let primary_time;
        let mut primary_action = 1;
        let mut secondary_time = None;
        let mut secondary_action = 0;
        let mut rdelay_mins_ = 0;
        match loc {
            Or(LocOr { sla, wta, wtd, .. }) | OpOr(LocOpOr { sla, wta, wtd, .. }) => {
                tiploc = sla.tpl;
                primary_time = wtd;
                secondary_time = wta;
            },
            Ip(LocIp { sla, wta, wtd, rdelay_mins, .. }) | OpIp(LocOpIp { sla, wta, wtd, rdelay_mins }) => {
                tiploc = sla.tpl;
                primary_time = wtd;
                secondary_time = Some(wta);
                rdelay_mins_ = rdelay_mins;
            },
            Pp(LocPp { sla, wtp, rdelay_mins }) => {
                tiploc = sla.tpl;
                primary_time = wtp;
                primary_action = 2;
                rdelay_mins_ = rdelay_mins;
            },
            Dt(LocDt { sla, wta, wtd, rdelay_mins, .. }) | OpDt(LocOpDt { sla, wta, wtd, rdelay_mins }) => {
                tiploc = sla.tpl;
                primary_time = wta;
                primary_action = 0;
                secondary_time = wtd;
                secondary_action = 1;
                rdelay_mins_ = rdelay_mins;
            },
        }
        mvts.push(ScheduleMvt {
            id: -1,
            parent_sched: -1,
            tiploc: tiploc.clone(),
            action: primary_action,
            origterm: false,
            time: primary_time + Duration::minutes(rdelay_mins_ as _),
            starts_path: None,
            ends_path: None
        });
        if let Some(time) = secondary_time {
            mvts.push(ScheduleMvt {
                id: -1,
                parent_sched: -1,
                tiploc: tiploc,
                action: secondary_action,
                origterm: false,
                time: time + Duration::minutes(rdelay_mins_ as _),
                starts_path: None,
                ends_path: None
            });
        }
    }
    mvts.sort_by_key(|mvt| mvt.time);
    let orig_mvts = ScheduleMvt::from_select(conn, "WHERE parent_sched = $1 ORDER BY time ASC", &[&train.parent_sched])?;
    if mvts == orig_mvts {
        worker.incr("darwin.sched.identical");
        debug!("Schedules are identical; doing nothing.");
        return Ok(());
    }
    let s = Schedule {
        uid: sched.uid.clone(),
        start_date: sched.ssd.clone(),
        end_date: sched.ssd.clone(),
        days: Days::all(),
        stp_indicator: StpIndicator::NewSchedule,
        source: 2,
        file_metaseq: None,
        geo_generation: 0,
        darwin_id: Some(sched.rid.clone()),
        signalling_id: Some(sched.train_id),
        id: -1
    };
    let (sid, update) = s.insert_self(conn)?;
    if update {
        // set of movements in old schedule = A (orig_mvts)
        // set of movements in new schedule = B (mvts)
        //
        // we want to delete A ∩ B' (in A but not in B)
        // ...do nothing to A ∩ B (in both)
        // ...and insert A' ∩ B (in B but not in A)
        let orig_mvts: HashSet<ScheduleMvt> = ScheduleMvt::from_select(conn, "WHERE parent_sched = $1 ORDER BY time ASC", &[&sid])?
            .into_iter().collect();
        let mvts: HashSet<ScheduleMvt> = mvts.into_iter().collect();
        let mut to_delete = vec![];
        for mvt in orig_mvts.difference(&mvts) {
            conn.execute("DELETE FROM schedule_movements WHERE id = $1", &[&mvt.id])?;
        }
        for mvt in mvts.difference(&orig_mvts) {
            let mut mvt = mvt.clone();
            mvt.parent_sched = sid;
            mvt.insert_self(conn)?;
        }
        debug!("Updated schedule #{}.", sid);
        worker.incr("darwin.sched.updated");
    }
    else {
        for mut mvt in mvts {
            mvt.parent_sched = sid;
            mvt.insert_self(conn)?;
        }
        debug!("Schedule inserted as #{}.", sid);
        worker.incr("darwin.sched.inserted");
    }
    conn.execute("UPDATE trains SET parent_nre_sched = $1 WHERE id = $2", &[&sid, &train.id])?;
    Ok(())
}
pub fn process_ts<T: GenericConnection>(conn: &T, worker: &mut NtrodWorker, ts: Ts) -> Result<()> {
    debug!("Processing update to rid {} (uid {}, start_date {})...", ts.rid, ts.uid, ts.start_date);
    let train = get_train_for_rid_uid_ssd(conn, worker, ts.rid, ts.uid, ts.start_date)?;
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
        let mvts = ScheduleMvt::from_select(conn, "WHERE parent_sched = $1 AND tiploc = $2 AND action = $3 AND time = $4 FOR KEY SHARE", &[&train.parent_sched, &tiploc, &action, &time])?;
        let mvt = match mvts.into_iter().nth(0) {
            Some(m) => m,
            None => {
                if let Some(darwin_sched) = train.parent_nre_sched {
                    debug!("Movement query failed - querying with Darwin parent_sched = {}", darwin_sched);
                    let mvts = ScheduleMvt::from_select(conn, "WHERE parent_sched = $1 AND tiploc = $2 AND action = $3 AND time = $4 FOR KEY SHARE", &[&darwin_sched, &tiploc, &action, &time])?;
                    match mvts.into_iter().nth(0) {
                        Some(m) => {
                            debug!("Found Darwin schedule movement #{}", m.id);
                            worker.incr("darwin.ts.darwin_sched_mvt");
                            m
                        },
                        None => {
                            debug!("Failed to find movements even with Darwin schedule (!)");
                            errs.push(NrodError::NoMovementsFoundDarwin(train.parent_sched, vec![action], vec![tiploc], Some(time)));
                            continue;
                        }
                    }
                }
                else {
                    errs.push(NrodError::NoMovementsFound(train.parent_sched, vec![action], vec![tiploc], Some(time)));
                    continue;
                }
            }
        };
        let actual = tstd.at.is_some();
        let time = tstd.at
            .or(tstd.wet)
            .or(tstd.et);
        let time = match time {
            Some(t) => t,
            None => {
                worker.incr("darwin.ts.no_useful_time");
                debug!("No useful time");
                continue;
            }
        };
        if tstd.at_removed {
            worker.incr("darwin.ts.at_removed");
            // TODO: make this source check less brittle
            conn.execute("DELETE FROM train_movements WHERE parent_mvt = $1 AND source = 1", &[&mvt.id])?;
        }
        if actual {
            worker.incr("darwin.ts.actual");
        }
        else {
            worker.incr("darwin.ts.estimated");
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
