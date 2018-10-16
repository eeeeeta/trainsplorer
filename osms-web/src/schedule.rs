use sctx::Sctx;
use tmpl::TemplateContext;
use osms_db::db::*;
use osms_db::ntrod::types::*;
use schedules;
use rouille::Response;

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
pub fn train(sctx: Sctx, id: i32) -> Response {
    let db = get_db!(sctx);
    let train = try_or_ise!(sctx, Train::from_select(&*db, "WHERE id = $1", &[&id]))
        .into_iter()
        .nth(0)
        .ok_or(format_err!("Couldn't find a train with id #{}.", id));
    let train = try_or_nonexistent!(sctx, train);
    let parent_sched = try_or_ise!(sctx, Schedule::from_select(&*db, "WHERE id = $1", &[&train.parent_sched]))
        .into_iter()
        .nth(0)
        .unwrap();
    let sched_mvts = try_or_ise!(sctx, ScheduleMvt::from_select(&*db, "WHERE parent_sched = $1 ORDER BY time ASC", &[&parent_sched.id]));
    let train_mvts = try_or_ise!(sctx, TrainMvt::from_select(&*db, "WHERE parent_train = $1", &[&train.id]));
    let mut descs = vec![];
    for mvt in sched_mvts {
        let action = action_to_str(mvt.action);
        let location = try_or_ise!(sctx, schedules::tiploc_to_readable(&*db, &mvt.tiploc));
        let (mut time_live, mut live_source) = (None, None);
        let mut trig = false;
        for tmvt in train_mvts.iter() {
            if tmvt.parent_mvt == mvt.id {
                if trig {
                    warn!("Duplicate train movement on train #{} for schedule mvt #{}", tmvt.parent_train, mvt.id);
                }
                time_live = Some(tmvt.time.to_string());
                live_source = Some(tmvt.source.clone());
                trig = true;
            }
        }
        let live_source = live_source.map(|x| {
            match x {
                0 => "TRUST",
                _ => "???"
            }.into()
        });
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
    render!(sctx, TemplateContext {
        template: "train",
        title: format!("Train #{}", train.id).into(),
        body: sd
    })
}
pub fn schedule(sctx: Sctx, id: i32) -> Response {
    let db = get_db!(sctx);
    let sched = try_or_ise!(sctx, Schedule::from_select(&*db, "WHERE id = $1", &[&id]))
        .into_iter()
        .nth(0)
        .ok_or(format_err!("Couldn't find a schedule with id #{}.", id));
    let sched = try_or_nonexistent!(sctx, sched);
    let movements = try_or_ise!(sctx, ScheduleMvt::from_select(&*db, "WHERE parent_sched = $1 ORDER BY time ASC", &[&id]));
    let mut descs = vec![];
    for mvt in movements {
        let action = action_to_str(mvt.action);
        let location = try_or_ise!(sctx, schedules::tiploc_to_readable(&*db, &mvt.tiploc));
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
    let trains_db = try_or_ise!(sctx, Train::from_select(&*db, "WHERE parent_sched = $1 ORDER BY date ASC", &[&id]));
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
    render!(sctx, TemplateContext {
        template: "schedule",
        title: format!("Schedule #{}", sched.id).into(),
        body: sd
    })
}
