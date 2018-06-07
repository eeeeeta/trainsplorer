use ntrod_types::movements::{Activation, Cancellation, Movement, Record, MvtBody, EventType};
use ntrod_types::reference::CorpusEntry;
use ntrod_types::vstp::Record as VstpRecord;
use osms_db::ntrod::types::*;
use osms_db::db::{DbType, InsertableDbType, GenericConnection};

type Result<T> = ::std::result::Result<T, ::failure::Error>;

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
                          WHERE uid = $1 AND start_date = $2 AND stp_indicator = $3",
                          &[&train_uid, &schedule_start_date, &stp_indicator])?;
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
                .ok_or(format_err!("no schedule segment"))?;
            let VstpScheduleSegment {
                schedule_location,
                signalling_id,
                ..
            } = schedule_segment;
            let sched = Schedule {
                uid: train_uid,
                start_date: schedule_start_date,
                end_date: schedule_end_date,
                days: schedule_days_runs,
                stp_indicator,
                signalling_id,
                geo_generation: 0,
                id: -1
            };
            let sid = sched.insert_self(&trans)?;
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
            for (tiploc, time, action, origterm) in mvts {
                let mvt = ScheduleMvt {
                    parent_sched: sid,
                    id: -1,
                    starts_path: None,
                    ends_path: None,
                    tiploc, time, action, origterm
                };
                mvt.insert_self(&trans)?;
                debug!("Schedule inserted as #{}.", sid);
            }
        }
    }
    trans.commit()?;
    Ok(())
}
pub fn process_ntrod_event<T: GenericConnection>(conn: &T, r: Record) -> Result<()> {
    let Record { header, body } = r;
    debug!("Processing message type {} from system {} (source {})",
           header.msg_type, header.source_system_id, header.original_data_source);
    match body {
        MvtBody::Activation(a) => process_activation(conn, a)?,
        MvtBody::Cancellation(a) => process_cancellation(conn, a)?,
        MvtBody::Movement(a) => process_movement(conn, a)?,
        MvtBody::Unknown(v) => {
            warn!("Got unknown value: {:?}", v);
            bail!("Unknown value received");
        },
        _ => {
            warn!("Don't know/care about message type {} yet!", header.msg_type);
            bail!("Unimplemented message type");
        }
    }
    Ok(())
}
pub fn process_activation<T: GenericConnection>(conn: &T, a: Activation) -> Result<()> {
    debug!("Processing activation of train {}...", a.train_id);
    let scheds = Schedule::from_select(conn,
        "WHERE uid = $1 AND stp_indicator = $2 AND start_date = $3",
        &[&a.train_uid, &a.schedule_type, &a.schedule_start_date])?;
    if scheds.len() == 0 {
        warn!("Failed to find a schedule.");
        bail!("Failed to find a schedule (UID {}, start {}, stp_indicator {:?})",
              a.train_uid, a.schedule_start_date, a.schedule_type);
    }
    let mut auth_schedule: Option<Schedule> = None;
    for sched in scheds {
        if !sched.is_authoritative(conn, a.origin_dep_timestamp.date())? {
            debug!("Schedule #{} is superseded.", sched.id);
        }
        else {
            if auth_schedule.is_some() {
                error!("Schedules #{} and #{} are both authoritative!",
                       sched.id, auth_schedule.as_ref().unwrap().id);
                bail!("Two authoritative schedules");
            }
            auth_schedule = Some(sched);
        }
    }
    let auth_schedule = if let Some(sch) = auth_schedule {
        sch
    }
    else {
        error!("No schedules are authoritative (UID {}, start {}, stp_indicator {:?})",
        a.train_uid, a.schedule_start_date, a.schedule_type);
        bail!("No authoritative schedules");
    };
    let train = Train {
        id: -1,
        parent_sched: auth_schedule.id,
        trust_id: a.train_id,
        date: a.origin_dep_timestamp.date(),
        signalling_id: a.schedule_wtt_id,
        cancelled: false,
        terminated: false
    };
    let id = train.insert_self(conn)?;
    debug!("Inserted train as #{}", id);
    Ok(())
}
pub fn process_cancellation<T: GenericConnection>(conn: &T, c: Cancellation) -> Result<()> {
    debug!("Processing cancellation of train {}...", c.train_id);
    conn.execute("UPDATE trains SET cancelled = true WHERE trust_id = $1 AND date = $2", &[&c.train_id, &c.dep_timestamp.date()])?;
    debug!("Train cancelled.");
    Ok(())
}
pub fn process_movement<T: GenericConnection>(conn: &T, m: Movement) -> Result<()> {
    debug!("Processing movement of train {} at STANOX {}...", m.train_id, m.loc_stanox);
    if m.train_terminated {
        debug!("Train has terminated.");
        conn.execute("UPDATE trains SET terminated = true WHERE trust_id = $1 AND (date = $2 OR date = ($2 - interval '1 day'))", &[&m.train_id, &m.actual_timestamp.date()])?;
    }
    if m.offroute_ind {
        debug!("Train #{} off route.", m.train_id);
        return Ok(());
    }
    let trains = Train::from_select(conn, "WHERE trust_id = $1 AND (date = $2 OR date = ($2 - interval '1 day'))", &[&m.train_id, &m.actual_timestamp.date()])?;
    let train = match trains.into_iter().nth(0) {
        Some(t) => t,
        None => bail!("No train found for ID {}", m.train_id)
    };
    let entries = CorpusEntry::from_select(conn, "WHERE stanox = $1 AND tiploc IS NOT NULL",
                                           &[&m.loc_stanox])?;
    let tiplocs = entries.into_iter().map(|x| x.tiploc.unwrap()).collect::<Vec<_>>();
    if tiplocs.len() == 0 {
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
    let mvts = ScheduleMvt::from_select(conn, "WHERE parent_sched = $1 AND tiploc = ANY($2) AND action = ANY($3) AND COALESCE(time = $4, TRUE) ORDER BY time ASC", &[&train.parent_sched, &tiplocs, &acceptable_actions, &m.planned_timestamp.map(|x| x.time())])?;
    if mvts.len() == 0 {
        bail!("no movements for sched {}, actions {:?}, tiplocs {:?}, time {:?}", train.parent_sched, acceptable_actions, tiplocs, m.planned_timestamp.map(|x| x.time()));
    }
    let mvt = mvts.into_iter().nth(0).unwrap();
    let tmvt = TrainMvt {
        id: -1,
        parent_train: train.id,
        parent_mvt: mvt.id,
        time: m.actual_timestamp.time(),
        source: 0
    };
    let id = tmvt.insert_self(conn)?;
    debug!("Registered train movement #{}.", id);
    Ok(())
}
