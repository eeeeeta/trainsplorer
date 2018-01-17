use ntrod_types::movements::{Activation, Cancellation, Movement, Record, MvtBody};
use ntrod_types::reference::CorpusEntry;
use osms_db::ntrod::types::*;
use osms_db::osm::types::*;
use osms_db::db::{DbType, InsertableDbType, GenericConnection};

type Result<T> = ::std::result::Result<T, ::failure::Error>;

pub fn process_ntrod_event<T: GenericConnection>(conn: &T, r: Record) -> Result<()> {
    let Record { header, body } = r;
    debug!("Processing message type {} from system {} (source {})",
           header.msg_type, header.source_system_id, header.original_data_source);
    match body {
        MvtBody::Activation(a) => process_activation(conn, a)?,
        MvtBody::Cancellation(a) => process_cancellation(conn, a)?,
        MvtBody::Movement(a) => process_movement(conn, a)?,
        _ => {
            warn!("Don't know/care about this type of message yet!");
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
        debug!("Failed to find a schedule.");
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
    let auth_schedule = auth_schedule.ok_or(format_err!("No authoritative schedules"))?;
    let ways = ScheduleWay::from_select(conn,
                                        "WHERE parent_id = $1",
                                        &[&auth_schedule.id])?;
    let train = Train {
        id: -1,
        from_id: auth_schedule.id,
        trust_id: a.train_id,
        date: a.origin_dep_timestamp.date(),
        signalling_id: a.schedule_wtt_id
    };
    let id = train.insert_self(conn)?;
    let mut new_ways = vec![];
    for mut way in ways {
        way.parent_id = None;
        way.train_id = Some(id);
        new_ways.push(way.insert_self(conn)?);
    }
    debug!("Inserted train as #{}", id);
    Ok(())
}
pub fn process_cancellation<T: GenericConnection>(conn: &T, c: Cancellation) -> Result<()> {
    debug!("Processing cancellation of train {}...", c.train_id);
    let trains = Train::from_select(conn, "WHERE trust_id = $1", &[&c.train_id])?;
    for train in trains {
        debug!("Processing train object #{}...", train.id);
        let ways = ScheduleWay::from_select(conn, "WHERE train_id = $1", &[&train.id])?;
        for way in ways {
            conn.execute("DELETE FROM schedule_ways WHERE id = $1", &[&way.id])?;
        }
    }
    debug!("Train cancelled.");
    Ok(())
}
pub fn process_movement<T: GenericConnection>(conn: &T, m: Movement) -> Result<()> {
    debug!("Processing movement of train {} at STANOX {}...", m.train_id, m.loc_stanox);
    if m.offroute_ind {
        debug!("Train #{} off route.", m.train_id);
        return Ok(());
    }
    let trains = Train::from_select(conn, "WHERE trust_id = $1", &[&m.train_id])?;
    let train = match trains.into_iter().nth(0) {
        Some(t) => t,
        None => bail!("No train found for ID {}", m.train_id)
    };
    let entries = CorpusEntry::from_select(conn, "WHERE stanox = $1 AND crs IS NOT NULL",
                                           &[&m.loc_stanox])?;
    let crs = match entries.into_iter().nth(0) {
        Some(c) => c.crs.unwrap(),
        None => bail!("No CRS found for STANOX {}", m.loc_stanox)
    };
    debug!("Found CRS: {}", crs);
    let ways = ScheduleWay::from_select(conn, "WHERE train_id = $1", &[&train.id])?;
    if ways.len() == 0 {
        bail!("No ways found for train {}", m.train_id);
    }
    let mut did_something = false;
    let mut delta = None;
    for way in ways {
        let sp = StationPath::from_select(conn, "WHERE id = $1", &[&way.station_path])?;
        let sp = sp.into_iter().nth(0).expect("Foreign key didn't do its job");
        if let Some(delta) = delta && way.source == 0 {
            debug!("Updating way #{} by delta of {}", way.id, delta);
            let new_start: ::chrono::NaiveTime = way.st + delta;
            let new_end: ::chrono::NaiveTime = way.et + delta;
            conn.execute("UPDATE schedule_ways SET st = $1, et = $2, source = 2 WHERE id = $3",
                         &[&new_start, &new_end, &way.id])?;
        }
        if sp.s1 == crs {
            did_something = true;
            debug!("Train movement matches way #{}'s start location!", way.id);
            let way_duration = way.et.signed_duration_since(way.st);
            let new_end = m.actual_timestamp + way_duration;
            delta = Some(m.actual_timestamp.time().signed_duration_since(way.st));
            conn.execute("UPDATE schedule_ways SET st = $1, et = $2, source = 1 WHERE id = $3",
                         &[&m.actual_timestamp.time(), &new_end.time(), &way.id])?;
        }
        if sp.s2 == crs {
            did_something = true;
            debug!("Train movement matches way #{}'s end location!", way.id);
            delta = Some(m.actual_timestamp.time().signed_duration_since(way.et));
            conn.execute("UPDATE schedule_ways SET et = $1, source = 3 WHERE id = $2",
                         &[&m.actual_timestamp.time(), &way.id])?;
        }
    }
    if !did_something {
        bail!("No way with CRS {} found for train {}", crs, m.train_id);
    }
    debug!("Train movement processed.");
    Ok(())
}
