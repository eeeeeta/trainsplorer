use super::Result;
use rocket_contrib::Template;
use pool::DbConn;
use tmpl::TemplateContext;
use osms_db::db::*;
use osms_db::ntrod::types::*;
use schedules;

#[derive(Serialize)]
pub struct ScheduleDesc {
    id: i32,
    movements: Vec<ScheduleMvtDesc>
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
    let sd = ScheduleDesc {
        id: sched.id,
        movements: descs
    };
    Ok(Template::render("schedule", TemplateContext {
        title: format!("Schedule #{}", sched.id).into(),
        body: sd
    }))
}
