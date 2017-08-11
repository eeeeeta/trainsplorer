#[macro_use] extern crate error_chain;
extern crate osm_signal;
extern crate postgres;
extern crate geo;
extern crate indicatif;
extern crate postgis;
extern crate ordered_float;

static ACCESS_TOKEN: &str = "[REDACTED]";
static DATABASE_URL: &str = "postgresql://eeeeeta@127.0.0.1/osm";
static HUXLEY_URL: &str = "https://huxley.apphb.com";

use std::collections::{HashSet, BTreeSet, HashMap};
use postgres::{Connection, GenericConnection, TlsMode};
use postgres::rows::Row;
use postgis::ewkb::{Point, LineString, LineStringT};
use osm_signal::*;
use ordered_float::OrderedFloat;
use indicatif::ProgressBar;
mod errors {
    error_chain! {
        links {
            Rail(::osm_signal::errors::RailError, ::osm_signal::errors::RailErrorKind);
        }
        foreign_links {
            Io(::std::io::Error);
            Postgres(::postgres::error::Error);
            PostgresConnect(::postgres::error::ConnectError);
        }
    }
}
use errors::*;
fn rail() -> Result<()> {
    let mut cli = RailClient::new(ACCESS_TOKEN, HUXLEY_URL)?;
    let mut servs = HashMap::new();
    let sb = cli.station_board("CLJ", 90)?;
    for serv in sb.train_services {
        if let Some(id) = serv.id {
            if let Some(rsid) = serv.rsid {
                servs.insert(rsid, id);
            }
        }
    }
    println!("[+] {} train services", servs.len());
    for (rsid, sid) in servs {
        let serv = cli.service(&sid)?;
        println!("{:#?}", serv);
    }
    Ok(())
}
#[derive(Clone, Debug)]
pub struct Extremity {
    osm_ids: Vec<i64>,
    name: Option<String>,
    way: LineString,
    generation: i32
}
impl Extremity {
    pub fn from_row(row: &Row) -> Self {
        Extremity {
            osm_ids: row.get(0),
            name: row.get(1),
            way: row.get(2),
            generation: row.get(3)
        }
    }
    pub fn append(mut self, other: Self) -> Self {
        self.osm_ids.extend(other.osm_ids);
        let name = if other.name.is_none() {
            self.name
        }
        else if self.name.is_none() {
            other.name
        }
        else {
            let o = other.name.unwrap();
            let s = self.name.unwrap();
            if s.contains(&o) { Some(s) }
            else {
                Some(format!("{} + {}", o, s))
            }
        };
        let srid = self.way.srid;
        let pts1 = self.way.points;
        let pts2 = other.way.points;
        let mut new_pts = vec![];
        for pt in pts1 {
            if pt == pts2[0] {
                break;
            }
            new_pts.push(pt);
        }
        new_pts.extend(pts2);

        Self {
            osm_ids: self.osm_ids,
            name: name,
            way: LineStringT {
                points: new_pts,
                srid
            },
            generation: self.generation + 1
        }
    }
    pub fn insert<T>(&self, conn: &T) -> Result<()> where T: GenericConnection {
        conn.execute("INSERT INTO extremities (osm_ids, name, way, generation) VALUES ($1, $2, $3, $4);",
                     &[&self.osm_ids, &self.name, &self.way, &self.generation])?;
        Ok(())
    }
}
fn osm() -> Result<()> {
    let conn = Connection::connect(DATABASE_URL, TlsMode::None)?;
    println!("[+] Creating tables...");
    conn.execute(r#"
CREATE TABLE IF NOT EXISTS extremities (
osm_ids BIGINT[] NOT NULL,
name VARCHAR,
way geometry NOT NULL,
generation INT NOT NULL,
used BOOL NOT NULL DEFAULT false
);"#, &[])?;
    let num_extremities: i64 = conn
        .query("SELECT COUNT(*) FROM extremities;", &[])?
        .into_iter()
        .nth(0)
        .unwrap()
        .get(0);
    let num_non_gen_extremities: i64 = conn
        .query("SELECT COUNT(*) FROM extremities WHERE generation != 1;", &[])?
        .into_iter()
        .nth(0)
        .unwrap()
        .get(0);
    println!("[+] {} extremities, {} of which generation != 1", num_extremities, num_non_gen_extremities);
    let run_conv;
    if num_non_gen_extremities != 0 {
        println!("[+] Previous run was interrupted. Deleting gen>1 extremities.");
        conn.execute("DELETE FROM extremities WHERE generation > 1;", &[])?;
        run_conv = false;
    }
    else if num_extremities == 0 {
        println!("[+] No extremities yet. Beginning way->extremity conversion...");
        run_conv = true;
    }
    else {
        println!("[+] All extremities OK.");
        run_conv = false;
    }
    if run_conv {
        let num_rows: i64 = conn
            .query("SELECT COUNT(*) FROM planet_osm_line WHERE railway IS NOT NULL;", &[])?
        .into_iter()
            .nth(0)
            .unwrap()
            .get(0);
        println!("[+] {} ways to process.", num_rows);
        let bar = ProgressBar::new(num_rows as _);
        let trans = conn.transaction()?;
        for row in &trans.query("SELECT osm_id, name, way, ST_EndPoint(way) FROM planet_osm_line WHERE railway IS NOT NULL;", &[])? {
            let extr = Extremity {
                osm_ids: vec![row.get(0)],
                name: row.get(1),
                way: row.get(2),
                generation: 1
            };
            if extr.osm_ids.len() > 0 {
                extr.insert(&trans)?;
            }
            bar.inc(1);
        }
        trans.commit()?;
        bar.finish();
    }
    println!("[+] Now for the hard part. This will take a looong time.");
    let mut generation: i32 = 1;
    loop {
        println!("[+] Processing generation {}", generation);
        let trans = conn.transaction()?;
        let num_extrs: i64 = conn
            .query("SELECT COUNT(*) FROM extremities WHERE generation = $1;", &[&generation])?
            .into_iter()
            .nth(0)
            .unwrap()
            .get(0);
        println!("[+] {} extremities to process", num_extrs);
        if num_extrs == 0 {
            println!("[+] We're done");
            break;
        }
        let bar = ProgressBar::new(num_extrs as _);
        for row in &trans.query("SELECT osm_ids, name, way, generation FROM extremities WHERE generation = $1", &[&generation])? {
            let mut extr = Extremity::from_row(&row);
            for row in &trans.query("UPDATE extremities SET used = true WHERE generation = $1 AND NOT osm_ids && $3 AND used = false AND ST_Intersects(ST_StartPoint(way), $2) AND ST_Intersects(ST_EndPoint($2), way) RETURNING osm_ids, name, way, generation", &[&generation, &extr.way, &extr.osm_ids])? {
                let extr2 = Extremity::from_row(&row);
                //println!("{:?}, {:?}", extr, extr2);
                let new_extr = extr.clone().append(extr2);
                //println!("{:?}", new_extr.osm_ids);
                //println!("{:?}", new_extr);
                new_extr.insert(&trans)?;
            }
            bar.inc(1);
        }
        trans.execute("UPDATE extremities SET used = false", &[])?;
        trans.commit()?;
        bar.finish();
        generation += 1;
    }
    Ok(())
}
quick_main!(osm);
