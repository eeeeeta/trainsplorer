extern crate postgres;
#[macro_use] extern crate error_chain;
extern crate osms_db;
extern crate clap;
extern crate env_logger;

use osms_db::*;
use osms_db::errors::*;
use postgres::{Connection, TlsMode};
use clap::{Arg, App};
use std::fs::File;
use std::io::BufReader;
fn example() -> Result<()> {
    env_logger::init().unwrap();
    let matches = App::new("osms-db importer")
        .author("eta <http://theta.eu.org>")
        .about("Imports NTROD data (CORPUS, SCHEDULE) into the database.")
        .arg(Arg::with_name("url")
             .short("u")
             .long("url")
             .value_name("postgresql://USER@IP/DBNAME")
             .required(true)
             .takes_value(true)
             .help("Sets the database URL to use."))
        .arg(Arg::with_name("file")
             .short("f")
             .long("file")
             .value_name("FILENAME")
             .takes_value(true)
             .help("Path to data to import (if 'corpus' or 'schedule')"))
        .arg(Arg::with_name("type")
             .short("t")
             .long("type")
             .value_name("{corpus, schedule}")
             .required(true)
             .takes_value(true)
             .help("Action to execute ('ways', 'corpus' or 'schedule')."))
        .get_matches();
    let url = matches.value_of("url").unwrap();
    let typ = matches.value_of("type").unwrap();
    let conn = Connection::connect(url, TlsMode::None).unwrap();
    if typ == "ways" {
        initialize_database(&conn)?;
        make_schedule_ways(&conn)?;
        return Ok(())
    }
    let file = matches.value_of("file").expect("File required for action, or invalid action");
    let file = File::open(file)?;
    let buf_reader = BufReader::new(file);
    initialize_database(&conn)?;
    match typ {
        "corpus" => import_corpus(&conn, buf_reader)?,
        "schedule" => apply_schedule_records(&conn, buf_reader)?,
        x => panic!("Invalid action '{}'", x)
    }
    Ok(())
}
quick_main!(example);
