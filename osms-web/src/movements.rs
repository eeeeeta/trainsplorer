use sctx::Sctx;
use tmpl::TemplateContext;
use osms_db::db::*;
use osms_db::ntrod::types::*;
use osms_db::ntrod;
use chrono::*;
use rouille::{Request, Response};
use std::collections::{BTreeMap, HashMap};
use schedules;
use schedule;
use templates::movements::{MovementsView, MovementDesc};
use templates::movement_search::MovementSearchView;

pub fn post_index_movements(sctx: Sctx, req: &Request) -> Response {
    let (mut tiploc, mut date, mut time) = (None, None, None);
    let input = try_or_badreq!(sctx, ::rouille::input::post::raw_urlencoded_post_input(req));
    for (k, v) in input {
        match &k as &str {
            "ts-tiploc" => {
                if v.trim().len() == 0 {
                    try_or_badreq!(sctx, Err(format_err!("Location cannot be blank.")));
                }
                else {
                    tiploc = Some(v);
                }
            },
            "ts-date" => {
                let d = NaiveDate::parse_from_str(&v, "%Y-%m-%d")
                    .map_err(|e| format_err!("Error parsing date: {}", e));
                date = Some(try_or_badreq!(sctx, d));
            },
            "ts-time" => {
                let t = NaiveTime::parse_from_str(&v, "%H:%M")
                    .map_err(|e| format_err!("Error parsing time: {}", e));
                time = Some(try_or_badreq!(sctx, t));
            },
            _ => {}
        }
    }
    let tiploc = try_or_badreq!(sctx, tiploc.ok_or(format_err!("A location is required.")));
    let date = try_or_badreq!(sctx, date.ok_or(format_err!("A date is required.")));
    let time = try_or_badreq!(sctx, time.ok_or(format_err!("A time is required.")));
    Response::redirect_303(format!("/movements/{}/{}/{}", tiploc, date, time))
}
#[derive(Serialize)]
pub struct StationSuggestion {
    name: String,
    code: String,
    code_type: String
}
#[derive(Serialize)]
pub struct StationSuggestions {
    suggestions: Vec<StationSuggestion>
}
pub fn station_suggestions(sctx: Sctx, req: &Request) -> Response {
    let db = get_db!(sctx);
    let query = req.get_param("query")
        .ok_or(format_err!("No suggestion query provided"));
    let query = try_or_badreq!(sctx, query);

    let mut tiplocs = HashMap::new();
    let mut similarity_tiplocs = BTreeMap::new();
    let mut crses = HashMap::new();
    let mut similarity_crses = BTreeMap::new();

    let msn_query = db.query("SELECT *, GREATEST(similarity(crs, $1), similarity(name, $1)), GREATEST(similarity(name, $1), similarity(tiploc, $1)) FROM msn_entries WHERE name % $1 OR crs % $1 OR tiploc % $1", &[&query]);

    for row in &try_or_ise!(sctx, msn_query) {
        let ent = MsnEntry::from_row(&row);
        let similarity_crs: f32 = row.get(4);
        let similarity_crs: u32 = (similarity_crs * 1000.0) as u32;
        let similarity_tiploc: f32 = row.get(5);
        let similarity_tiploc: u32 = (similarity_tiploc * 1000.0) as u32;
        if similarity_tiploc >= 300 && tiplocs.insert(ent.tiploc.clone(), ::titlecase::titlecase(&ent.name)).is_none() {
            similarity_tiplocs.insert(similarity_tiploc, ent.tiploc);
        }
        if similarity_crs >= 300 && crses.insert(ent.crs.clone(), ::titlecase::titlecase(&ent.name)).is_none() {
            similarity_crses.insert(similarity_crs, ent.crs);
        }
    }
    let corpus_query = db.query("SELECT *, GREATEST(similarity(nlcdesc, $1), similarity(tiploc, $1)), GREATEST(similarity(nlcdesc, $1), similarity(crs, $1)) FROM corpus_entries WHERE nlcdesc IS NOT NULL AND (crs IS NOT NULL or tiploc IS NOT NULL) AND (nlcdesc % $1 OR crs % $1 OR tiploc % $1)", &[&query]);

    for row in &try_or_ise!(sctx, corpus_query) {
        let ent = CorpusEntry::from_row(&row);
        let similarity_crs: f32 = row.get(7);
        let similarity_crs: u32 = (similarity_crs * 1000.0) as u32;
        let similarity_tiploc: f32 = row.get(8);
        let similarity_tiploc: u32 = (similarity_tiploc * 1000.0) as u32;
        if let Some(crs) = ent.crs {
            if similarity_crs >= 300 && crses.insert(crs.clone(), ::titlecase::titlecase(ent.nlcdesc.as_ref().unwrap())).is_none() {
                similarity_crses.insert(similarity_crs, crs);
            }
        }
        if let Some(tiploc) = ent.tiploc {
            if similarity_tiploc >= 300 && tiplocs.insert(tiploc.clone(), ::titlecase::titlecase(ent.nlcdesc.as_ref().unwrap())).is_none() {
                similarity_tiplocs.insert(similarity_tiploc, tiploc);
            }
        }
    }
    let mut ret = BTreeMap::new();
    for (si, crs) in similarity_crses.into_iter().rev().take(4) {
        let name = crses[&crs].clone();
        ret.insert(si + 1, StationSuggestion {
            name,
            code: crs,
            code_type: "CRS".into()
        });
    }
    for (si, tiploc) in similarity_tiplocs.into_iter().rev().take(4) {
        let name = tiplocs[&tiploc].clone();
        ret.insert(si, StationSuggestion {
            name,
            code: tiploc,
            code_type: "TIPLOC".into()
        });
    }
    let ret = ret.into_iter().rev().map(|(_, v)| v).collect();
    Response::json(&StationSuggestions {
        suggestions: ret
    })
}
pub fn movements(sctx: Sctx, station: String, date: String, time: String) -> Response {
    let db = get_db!(sctx);
    let mut tiplocs = try_or_ise!(sctx, MsnEntry::from_select(&*db, "WHERE crs = $1", &[&station]))
        .into_iter()
        .map(|x| x.tiploc)
        .collect::<Vec<_>>();
    tiplocs.push(station.clone());
    let date = try_or_badreq!(sctx, NaiveDate::parse_from_str(&date, "%Y-%m-%d"));
    let time = try_or_badreq!(sctx, NaiveTime::parse_from_str(&time, "%H:%M:%S"));
    let wd: i32 = date.weekday().number_from_monday() as _;
    let mvts = ScheduleMvt::from_select(&*db, "WHERE tiploc = ANY($1) AND (CAST($4 AS time) + interval '1 hour') >= time AND (CAST($4 AS time) - interval '1 hour') <= time AND EXISTS(SELECT * FROM schedules WHERE id = schedule_movements.parent_sched AND start_date <= $2 AND end_date >= $2 AND days_value_for_iso_weekday((days), $3) = true)", &[&tiplocs, &date, &wd, &time]);
    let mvts = try_or_ise!(sctx, mvts);
    let mvt_query_result = try_or_ise!(sctx, ntrod::mvt_query(&*db, &mvts, Some(date))); 
    let mut descs = vec![];
    for mvt in mvt_query_result.mvts {
        descs.push(MovementDesc {
            parent_sched: mvt.parent_sched,
            parent_train: mvt.parent_train,
            tiploc: mvt.tiploc,
            action: schedule::action_to_str(mvt.action),
            _action: mvt.action,
            time: mvt.time_scheduled.time.to_string(),
            _time: mvt.time_scheduled.time,
            time_expected: mvt.time_expected.map(|x| x.time.to_string()),
            time_actual: mvt.time_actual.map(|x| x.time.to_string()),
            canx: mvt.canx,
            orig_dest: try_or_ise!(sctx, schedules::ScheduleOrigDest::get_for_schedule(&*db, mvt.parent_sched))
        });
    }
    descs.sort_by_key(|d| (d._time, d._action));
    render!(sctx, TemplateContext {
        template: "movements",
        title: "Movement search".into(),
        body: MovementsView {
            mvts: descs,
            mvt_search: MovementSearchView {
                error: None,
                station: Some(station),
                date: date.format("%Y-%m-%d").to_string(),
                time: time.format("%H:%M").to_string()
            }
        }
    })
}
