use super::Result;
use rocket_contrib::Template;
use pool::DbConn;
use tmpl::TemplateContext;
use osms_db::db::*;
use osms_db::ntrod::types::*;
use schedules;

pub fn action_to_str(act: i32) -> &'static str {
    match act {
        0 => "arr",
        1 => "dep",
        2 => "pass",
        _ => "???"
    }
}
#[derive(Serialize)]
pub struct ScheduleDesc {
    movements: Vec<ScheduleMvtDesc>,
    trains: Vec<ScheduleTrainDesc>
}
#[derive(Serialize)]
pub struct TrainDesc {
    movements: Vec<ScheduleMvtDesc>
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
    let sched_mvts = ScheduleMvt::from_select(&*db, "WHERE parent_sched = $1 ORDER BY time ASC", &[&parent_sched.id])?;
    let train_mvts = TrainMvt::from_select(&*db, "WHERE parent_train = $1", &[&train.id])?;
    let mut descs = vec![];
    for mvt in sched_mvts {
        let action = action_to_str(mvt.action);
        let location = schedules::tiploc_to_readable(&*db, &mvt.tiploc)?;
        let (mut time_live, mut live_source) = (None, None);
        let mut trig = false;
        for tmvt in train_mvts.iter() {
            if tmvt.parent_mvt == mvt.id {
                if trig {
                    eprintln!("Duplicate train movement on train #{} for schedule mvt #{}", tmvt.parent_train, mvt.id);
                }
                time_live = Some(tmvt.time.to_string());
                live_source = Some(tmvt.source.clone());
                trig = true;
            }
        }
        descs.push(ScheduleMvtDesc {
            action,
            location,
            tiploc: mvt.tiploc,
            time_sched: mvt.time.to_string(),
            time_live,
            live_source,
            starts_path: mvt.starts_path,
            ends_path: mvt.ends_path
        });
    }
    let sd = TrainDesc {
        movements: descs,
    };
    Ok(Template::render("train", TemplateContext {
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
        let action = action_to_str(mvt.action);
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