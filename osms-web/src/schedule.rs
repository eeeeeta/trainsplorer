use sctx::Sctx;
use tmpl::TemplateContext;
use osms_db::db::*;
use osms_db::ntrod::types::*;
use osms_db::ntrod;
use schedules;
use rouille::Response;
use templates::schedule::{ScheduleView, ScheduleTrainDesc, ScheduleMvtDesc};
use templates::train::{TrainView, TrainMvtDesc};

pub fn action_to_str(act: i32) -> &'static str {
    match act {
        0 => "arr",
        1 => "dep",
        2 => "pass",
        _ => "???"
    }
}

pub fn train(sctx: Sctx, id: i32) -> Response {
    let db = get_db!(sctx);
    let train = try_or_ise!(sctx, Train::from_select(&*db, "WHERE id = $1", &[&id]))
        .into_iter().nth(0)
        .ok_or(format_err!("Couldn't find a train with id #{}.", id));
    let train = try_or_nonexistent!(sctx, train);

    let sched_mvts = try_or_ise!(sctx, ScheduleMvt::from_select(&*db, "WHERE parent_sched = $1 OR parent_sched = $2", &[&train.parent_sched, &train.parent_nre_sched]));
    let mut mvt_query_result = try_or_ise!(sctx, ntrod::mvt_query(&*db, &sched_mvts, Some(train.date)));

    let mut descs = vec![];
    mvt_query_result.mvts.sort_by_key(|m| (m.idx, m.time_scheduled.time, m.action));
    for mvt in mvt_query_result.mvts {
        descs.push(TrainMvtDesc {
            action: action_to_str(mvt.action),
            location: try_or_ise!(sctx, schedules::tiploc_to_readable(&*db, &mvt.tiploc)),
            tiploc: mvt.tiploc,
            time_scheduled: mvt.time_scheduled.time.to_string(),
            time_expected: mvt.time_expected.map(|x| x.time.to_string()),
            time_actual: mvt.time_actual.map(|x| x.time.to_string()),
            starts_path: mvt.starts_path,
            ends_path: mvt.ends_path,
        });
    }
    
    let sd = TrainView {
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
    let movements = try_or_ise!(sctx, ScheduleMvt::from_select(&*db, "WHERE parent_sched = $1", &[&id]));
    let mut mvt_query_result = try_or_ise!(sctx, ntrod::mvt_query(&*db, &movements, None));
    mvt_query_result.mvts.sort_by_key(|m| (m.idx, m.time_scheduled.time, m.action));

    let mut descs = vec![];
    for mvt in mvt_query_result.mvts {
        descs.push(ScheduleMvtDesc {
            action: action_to_str(mvt.action),
            location: try_or_ise!(sctx, schedules::tiploc_to_readable(&*db, &mvt.tiploc)),
            tiploc: mvt.tiploc,
            time_scheduled: mvt.time_scheduled.time.to_string(),
            starts_path: mvt.starts_path,
            ends_path: mvt.ends_path,
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
    let sd = ScheduleView {
        movements: descs,
        trains: trains
    };
    render!(sctx, TemplateContext {
        template: "schedule",
        title: format!("Schedule #{}", sched.id).into(),
        body: sd
    })
}
