#[macro_use] extern crate error_chain;
extern crate osms_db;
extern crate r2d2;
extern crate r2d2_postgres;
extern crate clap;
extern crate env_logger;

use osms_db::*;
use osms_db::errors::*;
use r2d2_postgres::{TlsMode, PostgresConnectionManager};
use clap::{Arg, App};
fn example() -> Result<()> {
    env_logger::init().unwrap();
    let matches = App::new("osms-db navigator")
        .author("eta <http://theta.eu.org>")
        .about("Spits out the GPX track between two CRS codes on the National Rail network.")
        .arg(Arg::with_name("url")
             .short("u")
             .long("url")
             .value_name("postgresql://USER@IP/DBNAME")
             .required(true)
             .takes_value(true)
             .help("Sets the database URL to use."))
        .arg(Arg::with_name("from")
             .short("f")
             .long("from")
             .value_name("STATION")
             .required(true)
             .takes_value(true)
             .help("Station to navigate from."))
        .arg(Arg::with_name("to")
             .short("t")
             .long("to")
             .value_name("STATION")
             .required(true)
             .takes_value(true)
             .help("Station to navigate to."))
        .get_matches();
    let url = matches.value_of("url").unwrap();
    let from = matches.value_of("from").unwrap();
    let to = matches.value_of("to").unwrap();
    let r2c = r2d2::Config::default();
    let manager = PostgresConnectionManager::new(url, TlsMode::None)
        .unwrap();
    let pool = r2d2::Pool::new(r2c, manager).unwrap();;
    db::initialize_database(&pool, 4)?;
    let sp = osm::navigate::navigate(&*pool.get().unwrap(), from, to)?;
    println!(r#"
<?xml version="1.0" encoding="UTF-8"?>
<gpx
 version="1.0"
creator="GPSBabel - http://www.gpsbabel.org"
xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
xmlns="http://www.topografix.com/GPX/1/0"
xsi:schemaLocation="http://www.topografix.com/GPX/1/0 http://www.topografix.com/GPX/1/0/gpx.xsd">
<trk>
<trkseg>
"#);
    for node in sp.way.points {
        println!(r#"<trkpt lat="{}" lon="{}" />"#, node.y, node.x);
    }
    println!(r#"
</trkseg>
</trk>
</gpx>"#);
    Ok(())
}
quick_main!(example);
