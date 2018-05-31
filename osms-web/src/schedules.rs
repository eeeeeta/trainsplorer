use super::Result;
use rocket_contrib::Template;
use pool::DbConn;
use qb::QueryBuilder;
use tmpl::TemplateContext;
use osms_db::db::DbType;
use osms_db::ntrod::types::*;

#[derive(Serialize, FromForm, Default, Clone)]
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
#[derive(Serialize)]
pub struct ScheduleRow {
    id: String,
    uid: String,
    stp: String,
    desc: String,
    start_date: String,
    end_date: String,
    days: String,
}
#[derive(Serialize)]
pub struct ScheduleView {
    schedules: Vec<ScheduleRow>,
    pagination: Option<i32>,
    more: Option<String>,
    date: Option<String>
}
#[get("/schedules?<opts>")]
pub fn schedules_qs(db: DbConn, opts: ScheduleOptions) -> Result<Template> {
    schedules(db, opts)
}
#[get("/schedules")]
pub fn schedules_noqs(db: DbConn) -> Result<Template> {
    schedules(db, Default::default())
}
fn schedules(db: DbConn, opts: ScheduleOptions) -> Result<Template> {
    use chrono::{Utc, NaiveDate};
    let mut curdate = None;
    let date = if let Some(ref d) = opts.date {
        Some(NaiveDate::parse_from_str(d, "%Y-%m-%d")?)
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
    let schedules = qb.query_dbtype::<Schedule, _>(&*db)?;
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
        let locs = ScheduleMvt::from_select(&*db, "WHERE parent_sched = $1", &[&sched.id])?;
        let desc = if locs.len() >= 2 {
            let s1 = MsnEntry::from_select(&*db, "WHERE tiploc = $1",
                                           &[&locs[0].tiploc])?;
            let s2 = MsnEntry::from_select(&*db, "WHERE tiploc = $1",
                                           &[&locs.last().unwrap().tiploc])?;
            format!("{} {} → {}", 
                    locs[0].time.format("%H:%M"),
                    s1.first().map(|x| &x.name).unwrap_or(&locs[0].tiploc),
                    s2.first().map(|x| &x.name).unwrap_or(&locs.last().unwrap().tiploc))
        } 
        else {
            "Cancellation or empty schedule".into()
        };
        rows.push(ScheduleRow {
            id: sched.id.to_string(),
            uid: sched.uid.clone(),
            stp: format!("{:?}", sched.stp_indicator),
            desc,
            start_date: sched.start_date.format("%Y-%m-%d").to_string(),
            end_date: sched.end_date.format("%Y-%m-%d").to_string(),
            days: sched.days.to_string()
        })
    }
    Ok(Template::render("schedules", TemplateContext {
        title: "Schedule search".into(),
        body: ScheduleView {
            schedules: rows,
            more,
            pagination,
            date: curdate
        }
    }))
}
