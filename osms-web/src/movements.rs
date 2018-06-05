use super::Result;
use rocket_contrib::Template;
use pool::DbConn;
use tmpl::TemplateContext;
use osms_db::db::*;
use osms_db::ntrod::types::*;
use chrono::*;
use schedules;
use schedule;

#[derive(Serialize)]
pub struct MovementsView {
    mvts: Vec<MovementDesc>
}
#[derive(Serialize)]
pub struct MovementDesc {
    parent_sched: Option<i32>,
    parent_train: Option<i32>,
    live: bool,
    prepped: bool,
    action: String,
    time: String,
    #[serde(skip_serializing)]
    _time: NaiveTime,
    orig_dest: schedules::ScheduleOrigDest
}

#[get("/movements/<station>/<date>/<time>")]
fn movements(db: DbConn, station: String, date: String, time: String) -> Result<Template> {
    let date = NaiveDate::parse_from_str(&date, "%Y-%m-%d")?;
    let time = NaiveTime::parse_from_str(&time, "%H:%M")?;
    let wd: i32 = date.weekday().number_from_monday() as _;
    let mvts = ScheduleMvt::from_select(&*db, "WHERE tiploc = $1 AND (CAST($4 AS time) + interval '1 hour') >= time AND (CAST($4 AS time) - interval '1 hour') <= time AND EXISTS(SELECT * FROM schedules WHERE id = schedule_movements.parent_sched AND start_date <= $2 AND end_date >= $2 AND days_value_for_iso_weekday((days), $3) = true)", &[&station, &date, &wd, &time])?;
    let ids = mvts.iter().map(|x| x.id).collect::<Vec<_>>();
    let train_mvts = TrainMvt::from_select(&*db, "WHERE parent_mvt = ANY($1) AND EXISTS(SELECT * FROM trains WHERE id = train_movements.parent_train AND date = $2)", &[&ids, &date])?; 
    let mut descs = vec![];
    for mvt in mvts {
        let sched = Schedule::from_select(&*db, "WHERE id = $1", &[&mvt.parent_sched])?
            .into_iter().nth(0).unwrap();
        if !sched.is_authoritative(&*db, date)? {
            continue;
        }
        let orig_dest = match schedules::ScheduleOrigDest::get_for_schedule(&*db, mvt.parent_sched)? {
            Some(od) => od,
            None => continue
        };
        let action = schedule::action_to_str(mvt.action);
        let mut parent_sched = Some(sched.id);
        let mut parent_train = None;
        let mut _time = mvt.time;
        let mut live = false;
        let mut prepped = false;
        for tmvt in train_mvts.iter() {
            if tmvt.parent_mvt == mvt.id {
                parent_train = Some(tmvt.parent_train);
                _time = tmvt.time;
                live = true;
            }
        }
        if parent_train.is_none() {
            for train in Train::from_select(&*db, "WHERE parent_sched = $1 AND date = $2", &[&mvt.parent_sched, &date])? {
                parent_train = Some(train.id);
                prepped = true;
            }
        }
        descs.push(MovementDesc {
            parent_sched,
            parent_train,
            time: _time.to_string(),
            _time,
            live,
            prepped,
            action: action.into(),
            orig_dest
        });
    }
    descs.sort_unstable_by(|a, b| a._time.cmp(&b._time));
    Ok(Template::render("movements", TemplateContext {
        title: "Movement search".into(),
        body: MovementsView {
            mvts: descs
        }
    }))
}
