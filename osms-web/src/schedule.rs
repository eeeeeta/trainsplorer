use sctx::Sctx;
use tmpl::TemplateContext;
use osms_db::db::*;
use osms_db::ntrod::types::*;
use osms_db::ntrod;
use schedules;
use rouille::Response;
use templates::schedule::{ScheduleView, ScheduleTrainDesc, ScheduleMvtDesc};
use templates::train::{TrainView, TrainMvtDesc};
use chrono::{Timelike, NaiveTime};

pub fn action_to_str(act: i32) -> &'static str {
    match act {
        0 => "arr",
        1 => "dep",
        2 => "pass",
        _ => "?"
    }
}
pub fn action_to_icon(act: i32) -> &'static str {
    match act {
        0 => "arrow-alt-circle-right",
        1 => "arrow-alt-circle-left",
        2 => "arrow-alt-circle-up",
        _ => "?"
    }
}
pub fn action_past_tense(act: i32) -> &'static str {
    match act {
        0 => "Arrived",
        1 => "Departed",
        2 => "Passed through",
        _ => "???"
    }
}
pub fn format_time_with_half(time: &NaiveTime) -> String {
    match time.second() {
        0 => time.format("%H:%M").to_string(),
        30 => time.format("%H:%M½").to_string(),
        _ => time.format("%H:%M:%S").to_string()
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
        let delayed = if let Some(te) = mvt.time_expected.as_ref().or(mvt.time_actual.as_ref()) {
            te.time > mvt.time_scheduled.time
        }
        else {
            false
        };
        descs.push(TrainMvtDesc {
            action: action_to_icon(mvt.action),
            action_past_tense: action_past_tense(mvt.action),
            delayed,
            location: try_or_ise!(sctx, schedules::tiploc_to_readable(&*db, &mvt.tiploc)),
            tiploc: mvt.tiploc,
            time_scheduled: format_time_with_half(&mvt.time_scheduled.time),
            time_expected: mvt.time_expected.map(|x| format_time_with_half(&x.time)),
            time_actual: mvt.time_actual.map(|x| format_time_with_half(&x.time)),
            starts_path: mvt.starts_path,
            ends_path: mvt.ends_path,
        });
    }
    let orig_dest = try_or_ise!(sctx, schedules::ScheduleOrigDest::get_for_schedule(&*db, train.parent_sched));
    let orig_dest = try_or_ise!(sctx, orig_dest.ok_or(format_err!("Train schedule origdest is empty")));
    let darwin_only = train.parent_nre_sched.is_some() && train.trust_id.is_none();
    let scheds = try_or_ise!(sctx, Schedule::from_select(&*db, "WHERE id = $1", &[&train.parent_sched]));
    let parent_sched = try_or_ise!(sctx, scheds.into_iter().nth(0).ok_or(format_err!("Couldn't find train parent schedule")));
    let title = format!("{} service from {} to {}", orig_dest.time, orig_dest.orig, orig_dest.dest);
    let sd = TrainView {
        movements: descs,
        trust_id: train.trust_id,
        parent_sched: train.parent_sched,
        date: train.date.to_string(),
        parent_nre_sched: train.parent_nre_sched,
        terminated: train.terminated,
        cancelled: train.cancelled,
        signalling_id: train.signalling_id,
        nre_id: train.nre_id,
        sched_uid: parent_sched.uid,
        orig_dest,
        darwin_only
    };
    render!(sctx, TemplateContext {
        template: "train",
        title: title.into(),
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
