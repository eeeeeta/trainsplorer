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
extern crate failure;
extern crate postgis;
extern crate url;
extern crate chrono;

pub mod pool;
use rocket_contrib::Template;
use rocket::fairing::AdHoc;
use std::path::{Path, PathBuf};
use rocket::response::NamedFile;

pub mod tmpl;
pub mod qb;
pub mod schedules;
pub mod mapping;
use tmpl::TemplateContext;

pub type Result<T> = ::std::result::Result<T, failure::Error>;

#[get("/<path..>", rank = 5)]
fn file_static(path: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new("static/").join(path)).ok()
}
#[get("/map")]
fn map() -> Template {
    Template::render("map", TemplateContext::title("Slippy map"))
}
#[get("/")]
fn index() -> Template {
    Template::render("index", TemplateContext::title("Home"))
}

fn main() {
    rocket::ignite()
        .attach(AdHoc::on_attach(|rocket| {
            println!("Setting up database connection...");
            Ok(pool::attach_db(rocket))
        }))
        .attach(Template::fairing())
        .mount("/", routes![
               index, 
               map,
               schedules::schedules_qs,
               schedules::schedules_noqs,
               mapping::geo_ways, 
               mapping::geo_stations,
               file_static
        ])
        .launch();
}
