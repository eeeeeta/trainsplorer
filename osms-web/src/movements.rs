use super::Result;
use rocket_contrib::Template;
use pool::DbConn;
use tmpl::TemplateContext;
use osms_db::db::*;
use osms_db::ntrod::types::*;
use chrono::*;
use rocket::response::{Flash, Redirect};
use rocket::request::Form;
use rocket_contrib::Json;
use std::collections::{BTreeMap, HashMap};

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
    canx: bool,
    action: String,
    time: String,
    #[serde(skip_serializing)]
    _time: NaiveTime,
    orig_dest: schedules::ScheduleOrigDest
}
#[derive(FromForm)]
pub struct MovementParams {
    #[form(field = "ts-tiploc")]
    tiploc: Option<String>,
    #[form(field = "ts-date")]
    date: Option<String>,
    #[form(field = "ts-time")]
    time: Option<String>
}
fn validate_movement_params(mvtp: MovementParams) -> Result<(String, String, String)> {
    let tiploc = mvtp.tiploc.ok_or(format_err!("A location is required."))?;
    if tiploc.trim().len() == 0 {
        return Err(format_err!("Location cannot be blank."))?;
    }
    let date = mvtp.date.ok_or(format_err!("A date is required."))?;
    if date.trim().len() == 0 {
        return Err(format_err!("Date cannot be blank."))?;
    }
    let time = mvtp.time.ok_or(format_err!("A time is required."))?;
    if time.trim().len() == 0 {
        return Err(format_err!("Time cannot be blank."))?;
    }
    NaiveDate::parse_from_str(&date, "%Y-%m-%d")
        .map_err(|e| format_err!("Error parsing date: {}", e))?;
    NaiveTime::parse_from_str(&time, "%H:%M")
        .map_err(|e| format_err!("Error parsing time: {}", e))?;
    Ok((tiploc, date, time))
}
#[post("/", data = "<mvtp>")]
fn post_index_movements(mvtp: Form<MovementParams>) -> ::std::result::Result<Redirect, Flash<Redirect>> {
    match validate_movement_params(mvtp.into_inner()) {
        Ok((tiploc, date, time)) => {
            Ok(Redirect::to(&format!("/movements/{}/{}/{}", tiploc, date, time)))
        },
        Err(e) => {
            Err(Flash::error(Redirect::to("/"), e.to_string()))
        }
    }
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
#[derive(FromForm)]
pub struct SuggestionOptions {
    query: String
}
#[get("/station_suggestions?<opts>")]
pub fn station_suggestions(db: DbConn, opts: SuggestionOptions) -> Result<Json<StationSuggestions>> {
    let mut tiplocs = HashMap::new();
    let mut similarity_tiplocs = BTreeMap::new();
    let mut crses = HashMap::new();
    let mut similarity_crses = BTreeMap::new();
    for row in &db.query("SELECT *, GREATEST(similarity(crs, $1), similarity(name, $1)), GREATEST(similarity(name, $1), similarity(tiploc, $1)) FROM msn_entries WHERE name % $1 OR crs % $1 OR tiploc % $1", &[&opts.query])? {
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
    for row in &db.query("SELECT *, GREATEST(similarity(nlcdesc, $1), similarity(tiploc, $1)), GREATEST(similarity(nlcdesc, $1), similarity(crs, $1)) FROM corpus_entries WHERE nlcdesc IS NOT NULL AND (crs IS NOT NULL or tiploc IS NOT NULL) AND (nlcdesc % $1 OR crs % $1 OR tiploc % $1)", &[&opts.query])? {
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
    Ok(Json(StationSuggestions {
        suggestions: ret
    }))
}
#[get("/movements/<station>/<date>/<time>")]
fn movements(db: DbConn, station: String, date: String, time: String) -> Result<Template> {
    let mut tiplocs = MsnEntry::from_select(&*db, "WHERE crs = $1", &[&station])?
        .into_iter()
        .map(|x| x.tiploc)
        .collect::<Vec<_>>();
    tiplocs.push(station);
    let date = NaiveDate::parse_from_str(&date, "%Y-%m-%d")?;
    let time = NaiveTime::parse_from_str(&time, "%H:%M")?;
    let wd: i32 = date.weekday().number_from_monday() as _;
    let mvts = ScheduleMvt::from_select(&*db, "WHERE tiploc = ANY($1) AND (CAST($4 AS time) + interval '1 hour') >= time AND (CAST($4 AS time) - interval '1 hour') <= time AND EXISTS(SELECT * FROM schedules WHERE id = schedule_movements.parent_sched AND start_date <= $2 AND end_date >= $2 AND days_value_for_iso_weekday((days), $3) = true)", &[&tiplocs, &date, &wd, &time])?;
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
        let mut canx = false;
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
                canx = train.cancelled;
            }
        }
        else {
            for train in Train::from_select(&*db, "WHERE id = $1", &[&parent_train.unwrap()])? {
                canx = train.cancelled;
            }
        }
        descs.push(MovementDesc {
            parent_sched,
            parent_train,
            time: _time.to_string(),
            _time,
            live,
            prepped,
            canx,
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
