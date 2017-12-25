extern crate osms_db;
extern crate fern;
extern crate toml;
extern crate r2d2;
extern crate r2d2_postgres;
extern crate flate2;
#[macro_use] extern crate error_chain;
#[macro_use] extern crate log;
#[macro_use] extern crate serde_derive;
extern crate osmpbfreader;
extern crate indicatif;
extern crate postgres;
extern crate geo;
extern crate postgis;
extern crate crossbeam;

use std::env;
use std::fs::File;
use std::io::{BufReader, Read};
use r2d2_postgres::{TlsMode, PostgresConnectionManager};
use postgres::tls::native_tls::NativeTls;
use osms_db::*;
use flate2::bufread::GzDecoder;
use osmpbfreader::OsmPbfReader;

pub mod errors {
    #![allow(unused_doc_comment)]
    error_chain! {
        links {
            OsmsDb(::osms_db::errors::Error, ::osms_db::errors::ErrorKind);
        }
        foreign_links {
            Pbf(::osmpbfreader::Error);
            Postgres(::postgres::error::Error);
        }
    }
}
pub mod make;

use errors::*;

#[derive(Deserialize)]
pub struct Config {
    database_url: String,
    require_tls: bool,
    map_data: String,
    corpus_data: String,
    schedule_data: String,
    threads: usize,
    #[serde(default)]
    db_only: bool,
    #[serde(default)]
    always_count: bool,
    #[serde(default)]
    limit_schedule_toc: Option<String>
}
fn run() -> Result<()> {
    fern::Dispatch::new()
        .format(|out, msg, record| {
            out.finish(format_args!("[{} {}] {}",
                                    record.target(),
                                    record.level(),
                                    msg))
        })
        .level(log::LogLevelFilter::Info)
        .level_for("osms_db_setup", log::LogLevelFilter::Debug)
        .level_for("osms_db", log::LogLevelFilter::Debug)
        .chain(std::io::stdout())
        .apply()
        .unwrap();
    info!("osms-db-setup tool starting");
    let args = env::args().skip(1).collect::<Vec<_>>();
    let path = args.get(0).map(|x| x as &str).unwrap_or("config.toml");
    info!("Loading config from file {}...", path);
    let mut file = File::open(path)
        .chain_err(|| "couldn't open config file")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .chain_err(|| "error reading from config file")?;
    info!("Parsing config...");
    let conf: Config = toml::de::from_str(&contents)
        .chain_err(|| "invalid config")?;

    info!("Opening data files...");
    let corpus = BufReader::new(File::open(conf.corpus_data)
        .chain_err(|| "couldn't open corpus data file")?);
    let schedule = BufReader::new(File::open(conf.schedule_data)
                                  .chain_err(|| "couldn't open schedule data file")?);
    let schedule = GzDecoder::new(schedule)
        .chain_err(|| "couldn't start gunzipping schedule data file")?;
    let mut map = OsmPbfReader::new(BufReader::new(File::open(conf.map_data)
                                               .chain_err(|| "couldn't open map data file")?));
    info!("Connecting to Postgres...");
    let r2c = r2d2::Config::default();
    let tls = if conf.require_tls {
        let tls = NativeTls::new()
            .chain_err(|| "couldn't initialize tls")?;
        TlsMode::Require(Box::new(tls))
    }
    else {
        TlsMode::None
    };
    let manager = PostgresConnectionManager::new(conf.database_url, tls)
        .chain_err(|| "couldn't connect to postgres")?;
    let pool = r2d2::Pool::new(r2c, manager).chain_err(|| "couldn't make db pool")?;

    let mut ctx = make::ImportContext::new(&mut map, &pool, conf.threads);
    if conf.always_count {
        info!("Counting objects in map file...");
        make::count(&mut ctx)?;
    }
    info!("Phase 1: initialising database");

    db::initialize_database(&*pool.get().unwrap())?;
    make::nodes(&mut ctx)?;
    make::links(&mut ctx)?;
    make::stations(&mut ctx)?;
    make::crossings(&mut ctx)?;
    make::separate_nodes(&mut ctx)?;
    if conf.db_only {
        info!("Database-only initialization specified; aborting!");
        return Ok(());
    }
    {
        let conn = pool.get().unwrap();
        if util::count(&*conn, "FROM corpus_entries", &[])? == 0 {
            info!("Phase 2: importing corpus data");
            ntrod::import_corpus(&*conn, corpus)?;
        }
        else {
            info!("Skipping phase 2: corpus data exists");
        }
        if util::count(&*conn, "FROM schedules", &[])? == 0 {
            info!("Phase 3: importing schedule records");
            ntrod::apply_schedule_records(&*conn, schedule,
                                          conf.limit_schedule_toc.as_ref().map(|x| x as &str))?;
        }
        else {
            info!("Skipping phase 3: schedule records exist");
        }
    }
    info!("Phase 4: making schedule ways");
    ntrod::make_schedule_ways(&pool, conf.threads)?;

    info!("Complete!");
    Ok(())
}
quick_main!(run);
