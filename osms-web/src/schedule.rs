use super::Result;
use rocket_contrib::Template;
use pool::DbConn;
use tmpl::TemplateContext;
use osms_db::db::*;
use osms_db::ntrod::types::*;
use schedules;
use chrono::NaiveTime;

#[derive(Serialize)]
pub struct ScheduleDesc {
    movements: Vec<ScheduleMvtDesc>,
    trains: Vec<ScheduleTrainDesc>
}
#[derive(Serialize)]
pub struct ScheduleTrainDesc {
    id: i32,
    date: String
}
#[derive(Serialize)]
pub struct ScheduleMvtDesc {
    action: &'static str,
    location: String,
    tiploc: String,
    time_sched: String,
    time_live: Option<String>,
    live_source: Option<String>,
    ends_path: Option<i32>,
    starts_path: Option<i32>
}
pub struct TrainJoinRow {
    tiploc: String,
    action: i32,
    time: NaiveTime,
    starts_path: Option<i32>,
    ends_path: Option<i32>,
    time_live: NaiveTime,
    live_source: String
}
#[get("/train/<id>")]
fn train(db: DbConn, id: i32) -> Result<Template> {
    let train = Train::from_select(&*db, "WHERE id = $1", &[&id])?
        .into_iter()
        .nth(0)
        .ok_or(format_err!("no train found"))?;
    let parent_sched = Schedule::from_select(&*db, "WHERE id = $1", &[&train.parent_sched])?
        .into_iter()
        .nth(0)
        .unwrap();
    let mut descs = vec![];
    for row in &db.query("SELECT schedule_movements.tiploc, schedule_movements.action, schedule_movements.time,
                                 schedule_movements.starts_path, schedule_movements.ends_path, train_movements.time,
                                 train_movements.source
                            FROM schedule_movements, train_movements
                           WHERE train_movements.parent_mvt = schedule_movements.id
                             AND train_movements.parent_train = $1
                             AND schedule_movements.parent_sched = $2
                        ORDER BY schedule_movements.time ASC", &[&train.id, &parent_sched.id])? {
        let tjr = TrainJoinRow {
            tiploc: row.get(0),
            action: row.get(1),
            time: row.get(2),
            starts_path: row.get(3),
            ends_path: row.get(4),
            time_live: row.get(5),
            live_source: row.get(6)
        };
        let action = match tjr.action {
            0 => "arr",
            1 => "dep",
            2 => "pass",
            _ => "???"
        };
        let location = schedules::tiploc_to_readable(&*db, &tjr.tiploc)?;
        descs.push(ScheduleMvtDesc {
            action,
            location,
            tiploc: tjr.tiploc,
            time_sched: tjr.time.to_string(),
            time_live: Some(tjr.time_live.to_string()),
            live_source: Some(tjr.live_source),
            starts_path: tjr.starts_path,
            ends_path: tjr.ends_path
        });
    }
    let sd = ScheduleDesc {
        movements: descs,
        trains: vec![]
    };
    Ok(Template::render("schedule", TemplateContext {
        title: format!("Train #{}", train.id).into(),
        body: sd
    }))
}
#[get("/schedule/<id>")]
fn schedule(db: DbConn, id: i32) -> Result<Template> {
    let sched = Schedule::from_select(&*db, "WHERE id = $1", &[&id])?
        .into_iter()
        .nth(0)
        .ok_or(format_err!("no schedule found"))?;
    let movements = ScheduleMvt::from_select(&*db, "WHERE parent_sched = $1 ORDER BY time ASC", &[&id])?;
    let mut descs = vec![];
    for mvt in movements {
        let action = match mvt.action {
            0 => "arr",
            1 => "dep",
            2 => "pass",
            _ => "???"
        };
        let location = schedules::tiploc_to_readable(&*db, &mvt.tiploc)?;
        descs.push(ScheduleMvtDesc {
            action,
            location,
            tiploc: mvt.tiploc,
            time_sched: mvt.time.to_string(),
            time_live: None,
            live_source: None,
            starts_path: mvt.starts_path,
            ends_path: mvt.ends_path
        });
    }
    let trains_db = Train::from_select(&*db, "WHERE parent_sched = $1 ORDER BY date ASC", &[&id])?;
    let mut trains = vec![];
    for trn in trains_db {
        trains.push(ScheduleTrainDesc {
            id: trn.id,
            date: trn.date.format("%Y-%m-%d").to_string()
        });
    }
    let sd = ScheduleDesc {
        movements: descs,
        trains: trains
    };
    Ok(Template::render("schedule", TemplateContext {
        title: format!("Schedule #{}", sched.id).into(),
        body: sd
    }))
}
