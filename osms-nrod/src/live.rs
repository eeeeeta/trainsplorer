use ntrod_types::movements::{Activation, Cancellation, Movement, Record, MvtBody, EventType, ScheduleSource};
use ntrod_types::reference::CorpusEntry;
use ntrod_types::vstp::Record as VstpRecord;
use osms_db::ntrod::types::*;
use chrono::Local;
use osms_db::db::{DbType, InsertableDbType, GenericConnection};
use errors::NrodError;
use super::NtrodWorker;

type Result<T> = ::std::result::Result<T, NrodError>;

pub fn process_vstp<T: GenericConnection>(conn: &T, r: VstpRecord) -> Result<()> {
    let trans = conn.transaction()?;
    use ntrod_types::vstp::*;
    use ntrod_types::vstp::VstpLocationRecord::*;

    let VstpRecord::V1(msg) = r;
    match msg.schedule {
        VstpScheduleRecord::Delete {
            train_uid,
            schedule_start_date,
            stp_indicator,
            ..
        } => {
            info!("Processing VSTP DELETE message id {} (UID {}, start {}, stp_indicator {:?})", msg.origin_msg_id, train_uid, schedule_start_date, stp_indicator);
            trans.execute("DELETE FROM schedules
                          WHERE uid = $1 AND start_date = $2 AND stp_indicator = $3 AND source = $4",
                          &[&train_uid, &schedule_start_date, &stp_indicator, &Schedule::SOURCE_VSTP])?;
        },
        VstpScheduleRecord::Create {
            train_uid,
            schedule_start_date,
            schedule_end_date,
            schedule_days_runs,
            stp_indicator,
            schedule_segment,
            ..
        } => {
            info!("Processing VSTP CREATE message id {} (UID {}, start {}, stp_indicator {:?})", msg.origin_msg_id, train_uid, schedule_start_date, stp_indicator);
            let schedule_segment = schedule_segment.into_iter().nth(0)
                .ok_or(NrodError::NoScheduleSegment)?;
            let VstpScheduleSegment {
                schedule_location,
                signalling_id,
                ..
            } = schedule_segment;
            let sched = Schedule {
                uid: train_uid.clone(),
                start_date: schedule_start_date,
                end_date: schedule_end_date,
                days: schedule_days_runs,
                stp_indicator,
                signalling_id,
                source: Schedule::SOURCE_VSTP,
                file_metaseq: None,
                geo_generation: 0,
                darwin_id: None,
                id: -1
            };
            let (sid, update) = sched.insert_self(&trans)?;
            if update {
                return Err(NrodError::DuplicateVstpSchedule {
                    train_uid,
                    start_date: schedule_start_date,
                    stp_indicator,
                    source: Schedule::SOURCE_VSTP
                });
            }
            let mut mvts = vec![];
            for loc in schedule_location {
                match loc {
                    Originating(VstpLocationRecordOriginating { location, scheduled_departure_time, .. }) => {
                        mvts.push((location.tiploc.tiploc_id, scheduled_departure_time, 1, true));
                    },
                    Intermediate(VstpLocationRecordIntermediate { location, scheduled_arrival_time, scheduled_departure_time, .. }) => {
                        mvts.push((location.tiploc.tiploc_id.clone(), scheduled_arrival_time, 0, false));
                        mvts.push((location.tiploc.tiploc_id, scheduled_departure_time, 1, false));
                    },
                    Pass(VstpLocationRecordPass { location, scheduled_pass_time, .. }) => {
                        mvts.push((location.tiploc.tiploc_id, scheduled_pass_time, 2, false));
                    },
                    Terminating(VstpLocationRecordTerminating { location, scheduled_arrival_time, .. }) => {
                        mvts.push((location.tiploc.tiploc_id, scheduled_arrival_time, 0, true));
                    }
                }
            }
            for (i, (tiploc, time, action, origterm)) in mvts.into_iter().enumerate() {
                let mvt = ScheduleMvt {
                    parent_sched: sid,
                    id: -1,
                    starts_path: None,
                    ends_path: None,
                    idx: Some(i as _),
                    tiploc, time, action, origterm
                };
                mvt.insert_self(&trans)?;
            }
            debug!("Schedule inserted as #{}.", sid);
        }
    }
    trans.execute("NOTIFY osms_schedule_updates;", &[])?;
    trans.commit()?;
    Ok(())
}
pub fn process_ntrod_event<T: GenericConnection>(conn: &T, worker: &mut NtrodWorker, r: Record) -> Result<()> {
    let Record { header, body } = r;
    debug!("Processing message type {} from system {} (source {})",
    header.msg_type, header.source_system_id, header.original_data_source);
    let now = Local::now().naive_local();
    if let Ok(dur) = now.signed_duration_since(header.msg_queue_timestamp).to_std() {
        worker.latency("nrod.latency", dur);
    }
    let trans = conn.transaction()?;
    match body {
        MvtBody::Activation(a) => process_activation(&trans, worker, a)?,
        MvtBody::Cancellation(a) => process_cancellation(&trans, a)?,
        MvtBody::Movement(a) => process_movement(&trans, worker, a)?,
        MvtBody::Unknown(v) => {
            return Err(NrodError::UnknownMvtBody(v));
        },
        _ => {
            return Err(NrodError::UnimplementedMessageType(header.msg_type));
        }
    }
    trans.commit()?;
    Ok(())
}
pub fn process_activation<T: GenericConnection>(conn: &T, worker: &mut NtrodWorker, a: Activation) -> Result<()> {
    debug!("Processing activation of train {}...", a.train_id);
    let src = match a.schedule_source {
        ScheduleSource::CifItps => Schedule::SOURCE_ITPS,
        ScheduleSource::VstpTops => Schedule::SOURCE_VSTP,
    };
    let scheds = Schedule::from_select(conn,
        "WHERE uid = $1 AND stp_indicator = $2 AND start_date = $3 AND source = $4",
        &[&a.train_uid, &a.schedule_type, &a.schedule_start_date, &src])?;
    if scheds.len() == 0 {
        debug!("Failed to find a schedule.");
        return Err(NrodError::NoSchedules {
            train_uid: a.train_uid,
            start_date: a.schedule_start_date,
            stp_indicator: a.schedule_type,
            source: src,
            train_id: a.train_id,
            date: a.origin_dep_timestamp.date()
        });
    }
    let mut auth_schedule: Option<Schedule> = None;
    for sched in scheds {
        if !sched.is_authoritative(conn, a.origin_dep_timestamp.date())? {
            debug!("Schedule #{} is superseded.", sched.id);
        }
        else {
            if auth_schedule.is_some() {
                return Err(NrodError::TwoAuthoritativeSchedules(sched.id, auth_schedule.as_ref().unwrap().id));
            }
            auth_schedule = Some(sched);
        }
    }
    let auth_schedule = if let Some(sch) = auth_schedule {
        sch
    }
    else {
        return Err(NrodError::NoAuthoritativeSchedules(a.train_uid, a.schedule_start_date, a.schedule_type, src));
    };
    let train = Train {
        id: -1,
        parent_sched: auth_schedule.id,
        trust_id: Some(a.train_id),
        date: a.origin_dep_timestamp.date(),
        signalling_id: Some(a.schedule_wtt_id),
        cancelled: false,
        terminated: false,
        nre_id: None,
        parent_nre_sched: None
    };
    let (train, was_update) = train.insert_self(conn)?;
    if was_update {
        worker.incr("nrod.activation.link_with_darwin");
        debug!("Linked pre-existing train #{} with NRE id {:?} to TRUST id {:?}", train.id, train.nre_id, train.trust_id);
    }
    else {
        worker.incr("nrod.activation.trust_only");
        debug!("Inserted train as #{}", train.id);
    }
    Ok(())
}
pub fn process_cancellation<T: GenericConnection>(conn: &T, c: Cancellation) -> Result<()> {
    debug!("Processing cancellation of train {}...", c.train_id);
    let trains = Train::from_select(conn, "WHERE trust_id = $1 AND (date = $2 OR date = ($2 - interval '1 day'))", &[&c.train_id, &c.canx_timestamp.date()])?;
    let train = match trains.into_iter().nth(0) {
        Some(t) => t,
        None => return Err(NrodError::NoTrainFound(c.train_id, c.canx_timestamp.date())),
    };
    debug!("Train being cancelled is train #{}", train.id);
    conn.execute("UPDATE trains SET cancelled = true WHERE id = $1", &[&train.id])?;
    conn.execute("DELETE FROM train_movements WHERE parent_train = $1 AND estimated = true", &[&train.id])?;
    debug!("Train cancelled.");
    Ok(())
}
pub fn process_movement<T: GenericConnection>(conn: &T, worker: &mut NtrodWorker, m: Movement) -> Result<()> {
    debug!("Processing movement of train {} at STANOX {}...", m.train_id, m.loc_stanox);
    if m.offroute_ind {
        worker.incr("nrod.mvt.offroute");
        debug!("Train #{} off route.", m.train_id);
        return Ok(());
    }
    let trains = Train::from_select(conn, "WHERE trust_id = $1 AND (date = $2 OR date = ($2 - interval '1 day'))", &[&m.train_id, &m.actual_timestamp.date()])?;
    let train = match trains.into_iter().nth(0) {
        Some(t) => t,
        None => return Err(NrodError::NoTrainFound(m.train_id, m.actual_timestamp.date())),
    };
    if m.train_terminated {
        worker.incr("nrod.mvt.terminated");
        debug!("Train has terminated.");
        conn.execute("UPDATE trains SET terminated = true WHERE id = $1", &[&train.id])?;
        conn.execute("DELETE FROM train_movements WHERE parent_train = $1 AND estimated = true", &[&train.id])?;
    }
    let entries = CorpusEntry::from_select(conn, "WHERE stanox = $1 AND tiploc IS NOT NULL",
                                           &[&m.loc_stanox])?;
    let tiplocs = entries.into_iter().map(|x| x.tiploc.unwrap()).collect::<Vec<_>>();
    if tiplocs.len() == 0 {
        worker.incr("nrod.mvt.unmatched_stanox");
        debug!("No TIPLOC found for STANOX {}", m.loc_stanox);
        return Ok(());
    }
    let action = match m.event_type {
        EventType::Arrival => 0,
        EventType::Destination => 0,
        EventType::Departure => 1
    };
    let acceptable_actions = vec![2, action];
    debug!("Mapped STANOX {} to TIPLOCs {:?}", m.loc_stanox, tiplocs);
    debug!("Querying for movements - parent_sched = {}, tiplocs = {:?}, actions = {:?}", train.parent_sched, tiplocs, acceptable_actions);
    let mvts = ScheduleMvt::from_select(conn, "WHERE parent_sched = $1 AND tiploc = ANY($2) AND action = ANY($3) AND COALESCE(time = $4, TRUE) ORDER BY (idx, time, action) ASC", &[&train.parent_sched, &tiplocs, &acceptable_actions, &m.planned_timestamp.map(|x| x.time())])?;
    if mvts.len() == 0 {
        return Err(NrodError::NoMovementsFound(train.parent_sched, acceptable_actions, tiplocs, m.planned_timestamp.map(|x| x.time())));
    }
    let mvt = mvts.into_iter().nth(0).unwrap();
    let delta = m.actual_timestamp.time().signed_duration_since(mvt.time);
    let tmvt = TrainMvt {
        id: -1,
        parent_train: train.id,
        parent_mvt: mvt.id,
        time: m.actual_timestamp.time(),
        source: MvtSource::SOURCE_TRUST,
        estimated: false
    };
    let id = tmvt.insert_self(conn)?;
    debug!("Registered train movement #{}.", id);
    worker.incr("nrod.mvt.actual");
    debug!("Deleting any estimations...");
    conn.execute("DELETE FROM train_movements WHERE parent_mvt = $1 AND source = 2 AND estimated = true", &[&mvt.id])?;
    debug!("Creating/updating naïve estimation movements with delta {}", delta);
    let remaining_sched_mvts = ScheduleMvt::from_select(conn, "WHERE parent_sched = $1 AND time > $2 ORDER BY (idx, time, action) ASC", &[&train.parent_sched, &mvt.time])?;
    for mvt in remaining_sched_mvts {
        let tmvt = TrainMvt {
            id: -1,
            parent_train: train.id,
            parent_mvt: mvt.id,
            time: mvt.time + delta,
            source: MvtSource::SOURCE_TRUST_NAIVE_ESTIMATION,
            estimated: true
        };
        let id = tmvt.insert_self(conn)?;
        worker.incr("nrod.mvt.estimated");
        debug!("Registered estimation train movement #{}.", id);
    }
    Ok(())
}
