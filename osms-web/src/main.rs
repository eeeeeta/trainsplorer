#![feature(plugin)]
#![plugin(rocket_codegen)]

extern crate rocket;
extern crate rocket_contrib;
extern crate postgres;
extern crate r2d2;
extern crate r2d2_postgres;
extern crate osms_db;

pub mod pool;
use pool::DbConn;
use rocket_contrib::Template;
use rocket::fairing::AdHoc;
use std::path::{Path, PathBuf};
use rocket::response::NamedFile;

#[get("/<path..>", rank = 5)]
fn file_static(path: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new("static/").join(path)).ok()
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
        .mount("/", routes![index, file_static])
        .launch();
}
