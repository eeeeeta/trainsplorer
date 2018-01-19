#![feature(plugin, custom_derive)]
#![plugin(rocket_codegen)]

extern crate rocket;
extern crate rocket_contrib;
extern crate postgres;
extern crate r2d2;
extern crate r2d2_postgres;
extern crate osms_db;
#[macro_use] extern crate serde_derive;
extern crate geojson;
#[macro_use] extern crate serde_json;
extern crate geo;
extern crate failure;
extern crate postgis;

pub mod pool;
use pool::DbConn;
use failure::Error;
use rocket_contrib::{Json, Template};
use rocket::fairing::AdHoc;
use std::path::{Path, PathBuf};
use rocket::response::NamedFile;
use geojson::{Feature, FeatureCollection};
use geo::*;
use postgis::ewkb::Point as PgPoint;
use postgis::ewkb::LineString as PgLineString;
use postgis::ewkb::Polygon as PgPolygon;
use osms_db::db::*;
use osms_db::util;
use osms_db::osm::types::*;
use osms_db::ntrod::types::*;

pub type Result<T> = ::std::result::Result<T, failure::Error>;

#[get("/<path..>", rank = 5)]
fn file_static(path: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new("static/").join(path)).ok()
}
#[derive(FromForm)]
struct GeoParameters {
    xmin: f64,
    xmax: f64,
    ymin: f64,
    ymax: f64,
    limit: Option<u32>
}
impl GeoParameters {
    fn as_bbox(&self) -> Bbox<f64> {
        Bbox {
            xmin: self.xmin,
            xmax: self.xmax,
            ymin: self.ymin,
            ymax: self.ymax
        }
    }
    fn area(&self) -> f64 {
        use geo::algorithm::area::Area;
        Area::area(&self.as_bbox())
    }
    fn as_poly(&self) -> Polygon<f64> {
        util::geo_bbox_to_poly(self.as_bbox())
    }
    fn as_pg_poly(&self) -> PgPolygon {
        util::geo_poly_to_postgis(self.as_poly())
    }
}

#[get("/geo/stations?<geo>")]
fn geo_stations(db: DbConn, geo: GeoParameters) -> Result<Json<FeatureCollection>> {
    let poly = geo.as_pg_poly();
    let mut limit = geo.limit.unwrap_or(500);
    if limit > 1000 {
        limit = 1000;
    }
    let mut features = vec![];
    for stn in Station::from_select(&*db, "WHERE ST_Intersects(area, $1) LIMIT $2", &[&poly, &(limit as i64)])? {
        let mut ls = vec![];
        for point in stn.area.rings[0].points.iter() {
            ls.push(vec![point.x, point.y]);
        }
        let geom = geojson::Geometry {
            bbox: None,
            value: geojson::Value::Polygon(vec![ls]),
            foreign_members: None
        };
        let mut props = serde_json::Map::new();
        props.insert("nr_ref".to_string(), json!(stn.nr_ref));
        features.push(Feature {
            properties: Some(props),
            bbox: None,
            geometry: Some(geom),
            id: None,
            foreign_members: None
        });
    }
    Ok(Json(FeatureCollection {
        bbox: None,
        features,
        foreign_members: None
    }))
}
#[get("/geo/ways?<geo>")]
fn geo_ways(db: DbConn, geo: GeoParameters) -> Result<Json<FeatureCollection>> {
    let poly = geo.as_pg_poly();
    let mut limit = geo.limit.unwrap_or(500);
    if limit > 1000 {
        limit = 1000;
    }
    let mut features = vec![];
    for link in Link::from_select(&*db, "WHERE ST_Intersects(way, $1) LIMIT $2", &[&poly, &(limit as i64)])? {
        let mut ls = vec![];
        for point in link.way.points {
            ls.push(vec![point.x, point.y]);
        }
        let geom = geojson::Geometry {
            bbox: None,
            value: geojson::Value::LineString(ls),
            foreign_members: None
        };
        let mut props = serde_json::Map::new();
        props.insert("p1".to_string(), json!(link.p1));
        props.insert("p2".to_string(), json!(link.p2));
        features.push(Feature {
            properties: Some(props),
            bbox: None,
            geometry: Some(geom),
            id: None,
            foreign_members: None
        });
    }
    Ok(Json(FeatureCollection {
        bbox: None,
        features,
        foreign_members: None
    }))
}
#[get("/")]
fn index() -> Template {
    Template::render("index", ())
}

fn main() {
    rocket::ignite()
        .attach(AdHoc::on_attach(|rocket| {
            println!("Setting up database connection...");
            Ok(pool::attach_db(rocket))
        }))
        .attach(Template::fairing())
        .mount("/", routes![index, geo_ways, geo_stations, file_static])
        .launch();
}
