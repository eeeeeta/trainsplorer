#![feature(plugin, custom_derive)]
#![plugin(rocket_codegen)]

extern crate rocket;
extern crate rocket_contrib;
extern crate postgres;
extern crate r2d2;
extern crate r2d2_postgres;
extern crate osms_db;
extern crate serde;
#[macro_use] extern crate serde_derive;
extern crate geojson;
#[macro_use] extern crate serde_json;
extern crate geo;
#[macro_use] extern crate failure;
extern crate postgis;
extern crate url;
extern crate chrono;
extern crate titlecase;

pub mod pool;
use rocket_contrib::Template;
use rocket::fairing::AdHoc;
use std::path::{Path, PathBuf};
use rocket::response::NamedFile;
use rocket::request::{Request, FlashMessage};
use chrono::*;

pub mod tmpl;
pub mod qb;
pub mod schedules;
pub mod schedule;
pub mod mapping;
pub mod movements;
use tmpl::TemplateContext;

pub type Result<T> = ::std::result::Result<T, failure::Error>;
#[derive(Serialize)]
pub struct NotFoundView {
    uri: String
}
#[derive(Serialize)]
pub struct MovementSearchView {
    error: Option<String>,
    station: Option<String>,
    date: String,
    time: String
}
#[derive(Serialize)]
pub struct IndexView {
    mvt_search: MovementSearchView
}

#[error(500)]
fn ise() -> Template {
    Template::render("ise", TemplateContext::title("500"))
}
#[error(404)]
fn not_found(req: &Request) -> Template {
    Template::render("not_found", TemplateContext {
        title: "404".into(),
        body: NotFoundView {
            uri: req.uri().to_string()
        }
    })
}
#[get("/<path..>", rank = 5)]
fn file_static(path: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new("static/").join(path)).ok()
}
#[get("/map")]
fn map() -> Template {
    Template::render("map", TemplateContext::title("Slippy map"))
}
#[get("/")]
fn index(fm: Option<FlashMessage>) -> Template {
    let now = Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    let time = now.format("%H:%M").to_string();
    let tctx = TemplateContext {
        title: "Home".into(),
        body: IndexView {
            mvt_search: MovementSearchView {
                error: fm.map(|x| x.msg().to_string()),
                station: None,
                date,
                time
            }
        }
    };
    Template::render("index", tctx)
}

fn main() {
    rocket::ignite()
        .attach(AdHoc::on_attach(|rocket| {
            println!("Setting up database connection...");
            Ok(pool::attach_db(rocket))
        }))
        .attach(Template::fairing())
        .catch(errors![not_found, ise])
        .mount("/", routes![
               index, 
               map,
               movements::post_index_movements,
               movements::movements,
               movements::station_suggestions,
               schedule::schedule,
               schedule::train,
               schedules::schedules_qs,
               schedules::schedules_noqs,
               mapping::geo_ways, 
               mapping::geo_stations,
               mapping::geo_correct_station,
               file_static
        ])
        .launch();
}
