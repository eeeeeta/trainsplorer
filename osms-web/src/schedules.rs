use super::Result;
use qb::QueryBuilder;
use tmpl::TemplateContext;
use osms_db::db::*;
use osms_db::util;
use osms_db::ntrod::types::*;
use sctx::Sctx;
use rouille::{Response, Request};

pub fn tiploc_to_readable<T: GenericConnection>(conn: &T, tl: &str) -> Result<String> {
    let msn = MsnEntry::from_select(conn, "WHERE tiploc = $1", &[&tl])?;
    let desc = match msn.into_iter().nth(0) {
        Some(e) => Some(e.name),
        None => {
            let ce = CorpusEntry::from_select(conn, "WHERE nlcdesc IS NOT NULL AND tiploc = $1", &[&tl])?;
            ce.into_iter().nth(0).map(|x| x.nlcdesc.unwrap())
        }
    };
    Ok(if let Some(d) = desc {
        ::titlecase::titlecase(&d)
    }
    else {
        format!("[TIPLOC {}]", tl)
    })
}
#[derive(Serialize, Default, Clone)]
pub struct ScheduleOptions {
    pagination: Option<i32>,
    uid: Option<String>,
    date: Option<String>
}
impl ScheduleOptions {
    fn as_qs(&self) -> String {
        use url::form_urlencoded::Serializer;

        let mut ser = Serializer::new(String::new());
        if let Some(p) = self.pagination {
            ser.append_pair("pagination", &p.to_string());
        }
        if let Some(ref u) = self.uid {
            ser.append_pair("uid", u);
        }
        if let Some(ref d) = self.date {
            ser.append_pair("date", d);
        }
        ser.finish()
    }
}
#[derive(Serialize, Debug)]
pub struct ScheduleOrigDest {
    time: String,
    orig: String,
    orig_tiploc: String,
    dest: String,
    dest_tiploc: String
}
impl ScheduleOrigDest {
    pub fn get_for_schedule<T: GenericConnection>(conn: &T, sid: i32) -> Result<Option<Self>> {
        let locs = ScheduleMvt::from_select(conn, "WHERE parent_sched = $1 ORDER BY time ASC", &[&sid])?;
        Ok(if locs.len() >= 2 {
            let orig_tiploc = &locs[0].tiploc;
            let dest_tiploc = &locs.last().unwrap().tiploc;
            let orig = tiploc_to_readable(conn, orig_tiploc)?;
            let dest = tiploc_to_readable(conn, dest_tiploc)?;
            let time = locs[0].time.format("%H:%M").to_string();
            Some(ScheduleOrigDest {
                orig_tiploc: orig_tiploc.to_owned(),
                dest_tiploc: dest_tiploc.to_owned(),
                orig, dest, time
            })
        } 
        else {
            None
        })
    }
}
#[derive(Serialize)]
pub struct ScheduleRow {
    id: String,
    uid: String,
    stp: String,
    orig_dest: Option<ScheduleOrigDest>,
    start_date: String,
    end_date: String,
    days: String,
    geo_generation: i32,
    n_trains: i64
}
#[derive(Serialize)]
pub struct ScheduleView {
    schedules: Vec<ScheduleRow>,
    pagination: Option<i32>,
    more: Option<String>,
    date: Option<String>
}
pub fn schedules(sctx: Sctx, req: &Request) -> Response {
    use chrono::{Utc, NaiveDate};
    let db = get_db!(sctx);
    let pagination = if let Some(p) = req.get_param("pagination") {
        let p = try_or_400!(p.parse());
        Some(p)
    }
    else {
        None
    };
    let opts = ScheduleOptions {
        pagination,
        uid: req.get_param("uid"),
        date: req.get_param("date")
    };

    let mut curdate = None;
    let date = if let Some(ref d) = opts.date {
        Some(try_or_400!(NaiveDate::parse_from_str(d, "%Y-%m-%d")))
    } else {
        curdate = Some(Utc::now().format("%Y-%m-%d").to_string());
        None 
    };
    let mut qb = QueryBuilder::new();
    if date.is_some() {
        qb.add("start_date <= $$", &[&date]);
        qb.add("end_date >= $$", &[&date]);
    }
    if opts.uid.is_some() {
        qb.add("uid = $$", &[&opts.uid]);
    }
    if opts.pagination.is_some() {
        qb.add("id < $$", &[&opts.pagination]);
    }
    qb.add_other("ORDER BY id DESC", &[]);
    qb.add_other("LIMIT 25", &[]);
    let schedules = try_or_ise!(sctx, qb.query_dbtype::<Schedule, _>(&*db));
    let mut more = None;
    let mut pagination = None;
    if let Some(p) = schedules.last() {
        if schedules.len() == 25 {
            let mut opts = opts.clone();
            pagination = Some(p.id);
            opts.pagination = Some(p.id);
            more = Some(opts.as_qs())
        }
    }
    let mut rows = vec![];
    for sched in schedules {
        let orig_dest = try_or_ise!(sctx, ScheduleOrigDest::get_for_schedule(&*db, sched.id));
        let n_trains = util::count(&*db, "FROM trains WHERE parent_sched = $1", &[&sched.id]);
        let n_trains = try_or_ise!(sctx, n_trains);

        rows.push(ScheduleRow {
            id: sched.id.to_string(),
            uid: sched.uid.clone(),
            stp: format!("{:?}", sched.stp_indicator),
            orig_dest,
            geo_generation: sched.geo_generation,
            start_date: sched.start_date.format("%Y-%m-%d").to_string(),
            end_date: sched.end_date.format("%Y-%m-%d").to_string(),
            days: sched.days.to_string(),
            n_trains
        })
    }
    render!(sctx, TemplateContext {
        template: "schedules",
        title: "Schedule search".into(),
        body: ScheduleView {
            schedules: rows,
            more,
            pagination,
            date: curdate
        }
    })
}
