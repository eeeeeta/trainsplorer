use postgres::GenericConnection;
use postgres::rows::Row;
use postgres::types::ToSql;
use postgis::ewkb::{Point, LineString, Polygon};
use chrono::*;
use ntrod_types::schedule::*;
use ntrod_types::reference::*;
use ntrod_types::cif::*;
use errors::*;
use geo;

pub trait DbType: Sized {
    fn table_name() -> &'static str;
    fn table_desc() -> &'static str;
    fn from_row(row: &Row) -> Self;
    fn make_table<T: GenericConnection>(conn: &T) -> Result<()> {
        conn.execute(&format!("CREATE TABLE IF NOT EXISTS {} ({})",
                             Self::table_name(), Self::table_desc()), &[])?;
        Ok(())
    }
    fn from_select<T: GenericConnection>(conn: &T, where_clause: &str, args: &[&ToSql]) -> Result<Vec<Self>> {
        let query = format!("SELECT * FROM {} {}", Self::table_name(), where_clause);
        let qry = conn.query(&query, args)?;
        let mut ret = vec![];
        for row in &qry {
            ret.push(Self::from_row(&row));
        }
        Ok(ret)
    }
}
pub trait InsertableDbType: DbType {
    type Id;
    fn insert_self<T: GenericConnection>(&self, conn: &T) -> Result<Self::Id>;
}
pub fn geo_pt_to_postgis(pt: geo::Point<f64>) -> Point {
    Point::new(pt.0.x, pt.0.y, Some(4326))
}
pub fn geo_ls_to_postgis(ls: geo::LineString<f64>) -> LineString {
    LineString {
        points: ls.0.into_iter().map(geo_pt_to_postgis).collect(),
        srid: Some(4326)
    }
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: i32,
    pub location: Point,
    pub distance: f32,
    pub parent: Option<i32>,
    pub visited: bool,
    pub graph_part: i32,
    pub parent_geom: Option<LineString>
}
impl DbType for Node {
    fn table_name() -> &'static str {
        "nodes"
    }
    fn table_desc() -> &'static str {
        r#"
id SERIAL PRIMARY KEY,
location geometry NOT NULL,
distance REAL NOT NULL DEFAULT 'Infinity',
parent INT,
visited BOOL NOT NULL DEFAULT false,
graph_part INT NOT NULL DEFAULT 0,
parent_geom geometry
"#
    }
    fn from_row(row: &Row) -> Self {
        Self {
            id: row.get(0),
            location: row.get(1),
            distance: row.get(2),
            parent: row.get(3),
            visited: row.get(4),
            graph_part: row.get(5),
            parent_geom: row.get(6)
        }
    }
}
impl Node {
    pub fn insert<T: GenericConnection>(conn: &T, location: Point) -> Result<i32> {
        for row in &conn.query("SELECT id FROM nodes WHERE location = $1",
                               &[&location])? {
            debug!("point {:?} already exists", location);
            return Ok(row.get(0));
        }
        let qry = conn.query("INSERT INTO nodes (location) VALUES ($1) RETURNING id",
                             &[&location])?;
        let mut ret = None;
        for row in &qry {
            ret = Some(row.get(0))
        }
        Ok(ret.expect("Somehow, we never got an id in Node::insert..."))
    }
    pub fn new_at_point<T: GenericConnection>(conn: &T, point: Point) -> Result<(i32, bool)> {
        use geo::algorithm::distance::Distance;
        use geo::algorithm::split::Split;
        use geo::algorithm::haversine_length::HaversineLength;
        let trans = conn.transaction()?;
        let pt = geo::Point::from_postgis(&point);
        let mut okay = false;
        let prev_nodes = Node::from_select(&trans, "WHERE location = $1", &[&point])?;
        if prev_nodes.len() > 0 {
            okay = true;
        }
        let node = Self::insert(&trans, point.clone())?;
        let qry = Link::from_select(&trans, "", &[])?;
        let mut links = vec![];
        for link in qry {
            let line = geo::LineString::from_postgis(&link.way);
            if line.distance(&pt) <= 0.00000001 {
                if line.0.first().map(|x| x == &pt).unwrap_or(false) ||
                    line.0.last().map(|x| x == &pt).unwrap_or(false) {
                    okay = true;
                    continue;
                }
                links.push(link);
            }
        }
        debug!("making new at point {:?}: {} links", point, links.len());
        for link in links {
            okay = true;
            let line = geo::LineString::from_postgis(&link.way);
            let vec = line.split(&pt, 0.00000001);
            if vec.len() != 2 {
                bail!("expected 2 results after split, got {}", vec.len());
            }
            let mut iter = vec.into_iter();
            let (first, last) = (iter.next().unwrap(), iter.next().unwrap());
            let (df, dl) = (first.haversine_length(), last.haversine_length());
            let (first, last) = (geo_ls_to_postgis(first), geo_ls_to_postgis(last));
            trans.execute(
                "UPDATE links SET p2 = $1, way = $2, distance = $3 WHERE p1 = $4 AND p2 = $5",
                &[&node, &first, &(df as f32), &link.p1, &link.p2]
            )?;
            let new = Link {
                p1: node,
                p2: link.p2,
                distance: dl as f32,
                way: last
            };
            debug!("new setup: {} <-> {} <-> {}", link.p1, node, link.p2);
            new.insert(&trans)?;
        }
        trans.commit()?;
        Ok((node, okay))
    }
}
#[derive(Debug, Clone)]
pub struct Station {
    pub nr_ref: String,
    pub point: i32,
    pub area: Polygon
}
impl DbType for Station {
    fn table_name() -> &'static str {
        "stations"
    }
    fn table_desc() -> &'static str {
        r#"
nr_ref VARCHAR UNIQUE NOT NULL,
point INT NOT NULL,
area geometry NOT NULL
"#
    }
    fn from_row(row: &Row) -> Self {
        Self {
            nr_ref: row.get(0),
            point: row.get(1),
            area: row.get(2),
        }
    }
}
impl Station {
    pub fn insert<T: GenericConnection>(conn: &T, nr_ref: &str, point: i32, area: Polygon) -> Result<()> {
        conn.execute("INSERT INTO stations (nr_ref, point, area) VALUES ($1, $2, $3)",
                     &[&nr_ref, &point, &area])?;
        Ok(())
    }

}
#[derive(Debug, Clone)]
pub struct Link {
    pub p1: i32,
    pub p2: i32,
    pub way: LineString,
    pub distance: f32
}
impl DbType for Link {
    fn table_name() -> &'static str {
        "links"
    }
    fn table_desc() -> &'static str {
        r#"
p1 INT NOT NULL,
p2 INT NOT NULL,
way geometry NOT NULL,
distance REAL NOT NULL
"#
    }
    fn from_row(row: &Row) -> Self {
        Self {
            p1: row.get(0),
            p2: row.get(1),
            way: row.get(2),
            distance: row.get(3)
        }
    }
}
impl Link {
    pub fn insert<T: GenericConnection>(&self, conn: &T) -> Result<()> {
        conn.execute("INSERT INTO links (p1, p2, way, distance) VALUES ($1, $2, $3, $4)",
                     &[&self.p1, &self.p2, &self.way, &self.distance])?;
        Ok(())
    }
}
#[derive(Debug, Clone)]
pub struct StationPath {
    pub s1: String,
    pub s2: String,
    pub way: LineString,
    pub nodes: Vec<i32>
}
impl DbType for StationPath {
    fn table_name() -> &'static str {
        "station_paths"
    }
    fn table_desc() -> &'static str {
        r#"
s1 VARCHAR NOT NULL,
s2 VARCHAR NOT NULL,
way geometry NOT NULL,
nodes INT[] NOT NULL,
PRIMARY KEY(s1, s2)
"#
    }
    fn from_row(row: &Row) -> Self {
        Self {
            s1: row.get(0),
            s2: row.get(1),
            way: row.get(2),
            nodes: row.get(3)
        }
    }
}
impl InsertableDbType for StationPath {
    type Id = ();
    fn insert_self<T: GenericConnection>(&self, conn: &T) -> Result<()> {
        conn.execute("INSERT INTO station_paths (s1, s2, way, nodes)
                      VALUES ($1, $2, $3, $4)
                      ON CONFLICT (s1, s2) DO UPDATE SET way = $3",
                     &[&self.s1, &self.s2, &self.way, &self.nodes])?;
        Ok(())
    }
}
#[derive(Debug, Clone)]
pub struct Schedule {
    pub uid: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub days: Days,
    pub stp_indicator: StpIndicator,
    pub signalling_id: Option<String>,
    pub locations: Vec<i32>,
    pub ways: Vec<i32>,
    pub id: i32,
}
impl Schedule {
    pub fn make_ways<T: GenericConnection>(&self, conn: &T) -> Result<()> {
        debug!("making ways for record (UID {}, start {}, stp_indicator {:?})",
               self.uid, self.start_date, self.stp_indicator);
        if self.ways.len() > 0 {
            bail!("Schedule ways already exist!");
        }
        let locs = ScheduleLocation::from_select(conn, "WHERE id = ANY($1)", &[&self.locations])?;
        if locs.len() != self.locations.len() {
            bail!("Inconsistency between db locs len ({}) and my len ({})",
                  locs.len(), self.locations.len());
        }
        let mut ways = vec![];
        let mut p1 = 0;
        'outer: loop {
            if p1 >= locs.len() { break; }
            if let Some(e1) = locs[p1].get_station(conn)? {
                for p2 in p1..locs.len() {
                    if let Some(e2) = locs[p2].get_station(conn)? {
                        if e1.nr_ref == e2.nr_ref {
                            continue;
                        }
                        let path = match super::navigate_cached(conn, &e1.nr_ref, &e2.nr_ref) {
                            Ok(x) => x,
                            Err(e) => {
                                error!("failed to navigate from {} to {}: {}",
                                       e1.nr_ref, e2.nr_ref, e);
                                continue;
                            }
                        };
                        debug!("made way from {} ({}) to {} ({})",
                               e1.nr_ref, locs[p1].time,
                               e2.nr_ref, locs[p2].time);
                        let sway = ScheduleWay {
                            p1: self.locations[p1],
                            p2: self.locations[p2],
                            st: locs[p1].time,
                            et: locs[p2].time,
                            way: path.way,
                            id: -1,
                            parent_id: self.id
                        };
                        ways.push(sway.insert_self(conn)?);
                        p1 = p2 + 1;
                        continue 'outer;
                    }
                }
            }
            p1 += 1;
        }
        conn.execute("UPDATE schedules SET ways = $1 WHERE id = $2",
                     &[&ways, &self.id])?;
        debug!("inserted {} ways", ways.len());
        Ok(())
    }
    pub fn apply_rec<T: GenericConnection>(conn: &T, rec: ScheduleRecord) -> Result<Option<i32>> {
        use self::LocationRecord::*;
        if let CreateOrDelete::Delete = rec.transaction_type {
            conn.execute("DELETE FROM schedules
                          WHERE uid = $1 AND start_date = $2 AND stp_indicator = $3",
                         &[&rec.train_uid, &rec.schedule_start_date.naive_utc(), &rec.stp_indicator])?;
            return Ok(None);
        }
        if let YesOrNo::N = rec.applicable_timetable {
            return Ok(None);
        }
        debug!("inserting record (UID {}, start {}, stp_indicator {:?})",
               rec.train_uid, rec.schedule_start_date, rec.stp_indicator);
        let ScheduleRecord {
            train_uid,
            schedule_start_date,
            schedule_end_date,
            schedule_days_runs,
            stp_indicator,
            schedule_segment,
            ..
        } = rec;
        let ScheduleSegment {
            schedule_location,
            signalling_id,
            ..
        } = schedule_segment;
        let mut sched = Schedule {
            uid: train_uid,
            start_date: schedule_start_date.naive_utc(),
            end_date: schedule_end_date.naive_utc(),
            days: schedule_days_runs,
            stp_indicator,
            signalling_id,
            locations: vec![],
            ways: vec![],
            id: -1
        };
        for loc in schedule_location {
            match loc {
                Originating { tiploc_code, departure, .. } => {
                    sched.locations.push(
                        ScheduleLocation::insert(conn, tiploc_code, departure, "originate")?);
                },
                Intermediate { tiploc_code, arrival, departure, .. } => {
                    sched.locations.push(
                        ScheduleLocation::insert(conn, tiploc_code.clone(), arrival, "arrive")?);
                    sched.locations.push(
                        ScheduleLocation::insert(conn, tiploc_code, departure, "depart")?);
                },
                Pass { tiploc_code, pass, .. } => {
                    sched.locations.push(
                        ScheduleLocation::insert(conn, tiploc_code, pass, "pass")?);
                },
                Terminating { tiploc_code, arrival, .. } => {
                    sched.locations.push(
                        ScheduleLocation::insert(conn, tiploc_code, arrival, "terminate")?);
                }
            }
        }
        Ok(Some(sched.insert_self(conn)?))
    }
}
impl InsertableDbType for Schedule {
    type Id = i32;
    fn insert_self<T: GenericConnection>(&self, conn: &T) -> Result<i32> {
        for row in &conn.query(
            "SELECT id FROM schedules
             WHERE uid = $1 AND start_date = $2 AND stp_indicator = $3",
                               &[&self.uid, &self.start_date, &self.stp_indicator])? {
            return Ok(row.get(0));
        }
        let qry = conn.query(
            "INSERT INTO schedules
             (uid, start_date, end_date, days, stp_indicator, signalling_id, locations, ways)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING id",
            &[&self.uid, &self.start_date, &self.end_date, &self.days, &self.stp_indicator,
              &self.signalling_id, &self.locations, &self.ways])?;
        let mut ret = None;
        for row in &qry {
            ret = Some(row.get(0))
        }
        Ok(ret.expect("No id in ScheduleLocation::insert?!"))
    }
}
impl DbType for Schedule {
    fn table_name() -> &'static str {
        "schedules"
    }
    fn table_desc() -> &'static str {
        r#"
uid VARCHAR NOT NULL,
start_date DATE NOT NULL,
end_date DATE NOT NULL,
days "Days" NOT NULL,
stp_indicator "StpIndicator" NOT NULL,
signalling_id VARCHAR,
locations INT[] NOT NULL,
ways INT[] NOT NULL,
id SERIAL UNIQUE NOT NULL,
PRIMARY KEY(uid, start_date, stp_indicator)
"#
    }
    fn from_row(row: &Row) -> Self {
        Self {
            uid: row.get(0),
            start_date: row.get(1),
            end_date: row.get(2),
            days: row.get(3),
            stp_indicator: row.get(4),
            signalling_id: row.get(5),
            locations: row.get(6),
            ways: row.get(7),
            id: row.get(8)
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScheduleLocation {
    pub id: i32,
    pub tiploc: String,
    pub time: NaiveTime,
    pub event: String
}
impl DbType for ScheduleLocation {
    fn table_name() -> &'static str {
        "schedule_locs"
    }
    fn table_desc() -> &'static str {
        r#"
id SERIAL PRIMARY KEY,
tiploc VARCHAR NOT NULL,
time TIME NOT NULL,
event VARCHAR NOT NULL
"#
    }
    fn from_row(row: &Row) -> Self {
        Self {
            id: row.get(0),
            tiploc: row.get(1),
            time: row.get(2),
            event: row.get(3)
        }
    }
}
impl ScheduleLocation {
    pub fn get_station<T: GenericConnection>(&self, conn: &T) -> Result<Option<Station>> {
        Ok(if let Some(crs) = self.get_crs(conn)? {
            let stats = Station::from_select(conn, "WHERE nr_ref = $1", &[&crs])?;
            if stats.len() == 0 {
                trace!("No station for CRS {}", crs);
            }
            stats.into_iter()
                .nth(0)
        }
        else {
            None
        })
    }
    pub fn get_crs<T: GenericConnection>(&self, conn: &T) -> Result<Option<String>> {
        let entries = CorpusEntry::from_select(conn,
                                               "WHERE tiploc = $1 AND crs IS NOT NULL",
                                               &[&self.tiploc])?;
        let mut ret = None;
        for ent in entries {
            if ent.crs.is_some() {
                ret = ent.crs;
                break;
            }
        }
        if ret.is_none() {
            trace!("Could not find a CRS for TIPLOC {}", self.tiploc);
        }
        Ok(ret)
    }
    pub fn insert<T: GenericConnection, U: Into<String>>(conn: &T, tiploc: String, time: NaiveTime, event: U) -> Result<i32> {
        let qry = conn.query(
            "INSERT INTO schedule_locs (tiploc, time, event)
             VALUES ($1, $2, $3)
             RETURNING id",
            &[&tiploc, &time, &event.into()])?;
        let mut ret = None;
        for row in &qry {
            ret = Some(row.get(0))
        }
        Ok(ret.expect("No id in ScheduleLocation::insert?!"))
    }
}
#[derive(Debug, Clone)]
pub struct Train {
    pub id: i32,
    pub from_uid: String,
    pub date: NaiveDate,
    pub signalling_id: String,
    pub ways: Vec<i32>
}
impl DbType for Train {
    fn table_name() -> &'static str {
        "trains"
    }
    fn table_desc() -> &'static str {
        r#"
id SERIAL PRIMARY KEY,
from_uid VARCHAR NOT NULL,
date DATE NOT NULL,
signalling_id VARCHAR NOT NULL,
ways INT[] NOT NULL
"#
    }
    fn from_row(row: &Row) -> Self {
        Self {
            id: row.get(0),
            from_uid: row.get(1),
            date: row.get(2),
            signalling_id: row.get(3),
            ways: row.get(4)
        }
    }
}
#[derive(Debug, Clone)]
pub struct ScheduleWay {
    pub p1: i32,
    pub p2: i32,
    pub st: NaiveTime,
    pub et: NaiveTime,
    pub way: LineString,
    pub id: i32,
    pub parent_id: i32,
}
impl DbType for ScheduleWay {
    fn table_name() -> &'static str {
        "schedule_ways"
    }
    fn table_desc() -> &'static str {
        r#"
p1 INT NOT NULL,
p2 INT NOT NULL,
st TIME NOT NULL,
et TIME NOT NULL,
way geometry NOT NULL,
id SERIAL UNIQUE,
parent_id INT NOT NULL,
PRIMARY KEY(p1, p2)
"#
    }
    fn from_row(row: &Row) -> Self {
        Self {
            p1: row.get(0),
            p2: row.get(1),
            st: row.get(2),
            et: row.get(3),
            way: row.get(4),
            id: row.get(5),
            parent_id: row.get(6),
        }
    }
}
impl InsertableDbType for ScheduleWay {
    type Id = i32;
    fn insert_self<T: GenericConnection>(&self, conn: &T) -> Result<i32> {
        for row in &conn.query(
            "SELECT id FROM schedule_ways
             WHERE p1 = $1 AND p2 = $2",
            &[&self.p1, &self.p2])? {
            return Ok(row.get(0));
        }
        let qry = conn.query("INSERT INTO schedule_ways
                              (p1, p2, st, et, way, parent_id)
                              VALUES ($1, $2, $3, $4, $5, $6)
                              RETURNING id",
                             &[&self.p1, &self.p2, &self.st, &self.et,
                               &self.way, &self.parent_id])?;
        let mut ret = None;
        for row in &qry {
            ret = Some(row.get(0))
        }
        Ok(ret.expect("No id in ScheduleWay::insert?!"))
    }
}
impl InsertableDbType for CorpusEntry {
    type Id = ();
    fn insert_self<T: GenericConnection>(&self, conn: &T) -> Result<()> {
        conn.execute("INSERT INTO corpus_entries
                      (stanox, uic, crs, tiploc, nlc, nlcdesc, nlcdesc16)
                      VALUES ($1, $2, $3, $4, $5, $6, $7)
                      ON CONFLICT DO NOTHING",
                     &[&self.stanox, &self.uic, &self.crs, &self.tiploc, &self.nlc,
                       &self.nlcdesc, &self.nlcdesc16])?;
        Ok(())
    }
}
impl DbType for CorpusEntry {
    fn table_name() -> &'static str {
        "corpus_entries"
    }
    fn table_desc() -> &'static str {
        r#"
stanox VARCHAR UNIQUE,
uic VARCHAR UNIQUE,
crs VARCHAR UNIQUE,
tiploc VARCHAR UNIQUE,
nlc VARCHAR UNIQUE,
nlcdesc VARCHAR,
nlcdesc16 VARCHAR
"#
    }
    fn from_row(row: &Row) -> Self {
        Self {
            stanox: row.get(0),
            uic: row.get(1),
            crs: row.get(2),
            tiploc: row.get(3),
            nlc: row.get(4),
            nlcdesc: row.get(5),
            nlcdesc16: row.get(6),
        }
    }
}
