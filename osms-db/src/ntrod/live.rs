use ntrod_types::movements::*;
use super::types::*;
use db::{DbType, InsertableDbType, GenericConnection};
use errors::*;

pub fn process_activation<T: GenericConnection>(conn: &T, a: Activation) -> Result<()> {
    debug!("Processing activation of train {}...", a.train_id);
    let scheds = Schedule::from_select(conn,
        "WHERE uid = $1 AND stp_indicator = $2 AND start_date = $3",
        &[&a.train_uid, &a.schedule_type, &a.schedule_start_date])?;
    if scheds.len() == 0 {
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
    let auth_schedule = auth_schedule.ok_or("No authoritative schedules")?;
    let ways = ScheduleWay::from_select(conn,
                                        "WHERE parent_id = $1",
                                        &[&auth_schedule.id])?;
    let mut new_ways = vec![];
    for way in ways {
        new_ways.push(way.insert_self(conn)?);
    }
    let train = Train {
        id: -1,
        from_id: auth_schedule.id,
        trust_id: a.train_id,
        date: a.origin_dep_timestamp.date(),
        signalling_id: a.schedule_wtt_id,
        ways: new_ways
    };
    let id = train.insert_self(conn)?;
    debug!("Inserted train as #{}", id);
    Ok(())
}
pub fn process_cancellation<T: GenericConnection>(conn: &T, c: Cancellation) -> Result<()> {
    debug!("Processing cancellation of train {}...", c.train_id);
    let trains = Train::from_select("WHERE train_id = $1", &[&c.train_id])?;
    Ok(())
}
