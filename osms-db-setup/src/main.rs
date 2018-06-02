extern crate osms_db;
extern crate fern;
extern crate toml;
extern crate r2d2;
extern crate r2d2_postgres;
extern crate flate2;
#[macro_use] extern crate failure;
#[macro_use] extern crate log;
#[macro_use] extern crate serde_derive;
extern crate osmpbfreader;
extern crate indicatif;
extern crate postgres;
extern crate geo;
extern crate postgis;
extern crate crossbeam;
extern crate clap;
extern crate chrono;
extern crate reqwest;
extern crate csv;
extern crate atoc_msn;
extern crate ntrod_types;
extern crate serde_json;

use clap::{Arg, App, SubCommand, AppSettings};
use std::fs::File;
use std::io::{BufReader, Read};
use r2d2_postgres::{TlsMode, PostgresConnectionManager};
use postgres::tls::native_tls::NativeTls;
use osms_db::*;
use flate2::bufread::GzDecoder;
use osmpbfreader::OsmPbfReader;
use failure::{Error, ResultExt, err_msg};
use reqwest::{Response, Client};
use reqwest::header::{Authorization, Basic};

pub mod make;

#[derive(Deserialize)]
pub struct Config {
    database_url: String,
    nrod_user: String,
    nrod_pass: String,
    require_tls: bool,
    threads: usize,
}
fn download(url: &str, cli: &mut Client, user: &str, pass: &str) -> Result<Response, Error> {
    let creds = Basic {
        username: user.to_string(),
        password: Some(pass.to_string())
    };
    let resp = cli.get(url)
        .header(Authorization(creds))
        .send()?;
    Ok(resp)
}
fn run() -> Result<(), Error> {
    let matches = App::new("osms-db-setup")
        .version(env!("CARGO_PKG_VERSION"))
        .author("eta <https://theta.eu.org>")
        .about("Sets up and administers the database for the osm-signal project.")
        .setting(AppSettings::SubcommandRequired)
        .arg(Arg::with_name("config")
             .short("c")
             .long("config")
             .value_name("FILE")
             .help("Path to a configuration file (default 'config.toml')")
             .takes_value(true))
        .arg(Arg::with_name("logfile")
             .short("l")
             .long("logfile")
             .value_name("FILE")
             .help("Path to an optional logfile to write log messages to.")
             .takes_value(true))
        .subcommand(SubCommand::with_name("setup")
                    .about("Sets up a blank database from scratch. Usually, run `init` first, then `osm` and `nrod`.")
                    .setting(AppSettings::SubcommandRequired)
                    .subcommand(SubCommand::with_name("init")
                                .about("Initialises a blank database, setting up tables & relations."))
                    .subcommand(SubCommand::with_name("osm")
                                .about("Imports data from OpenStreetMap.")
                                .arg(Arg::with_name("count")
                                     .short("n")
                                     .long("count")
                                     .help("Always count the number of OSM objects in the file before starting. Results in better progress bars, but takes a bit longer."))
                                .arg(Arg::with_name("mapdata")
                                     .short("m")
                                     .long("mapdata")
                                     .value_name("FILE")
                                     .required(true)
                                     .takes_value(true)
                                     .help("Path to a PBF-format file to import map data from.")))
                    .subcommand(SubCommand::with_name("nrod")
                                .about("Imports reference data from Network Rail.")
                                .arg(Arg::with_name("msn")
                                     .short("m")
                                     .long("msn")
                                     .value_name("FILE")
                                     .takes_value(true)
                                     .required(true)
                                     .help("Path to ttisf786.msn from ATOC."))
                                .arg(Arg::with_name("naptan")
                                     .short("a")
                                     .long("naptan")
                                     .value_name("FILE")
                                     .takes_value(true)
                                     .required(true)
                                     .help("Path to RailReferences.csv from the NAPTAN data."))
                                .arg(Arg::with_name("corpus")
                                     .short("r")
                                     .long("corpus")
                                     .value_name("FILE")
                                     .required(true)
                                     .takes_value(true)
                                     .help("Path to CORPUSExtract.json.gz from Network Rail.")))
        )
        .subcommand(SubCommand::with_name("schedule")
                    .about("Performs operations related to scheduling. Run 'init' once after completing 'setup', then run 'update' daily.")
                    .setting(AppSettings::SubcommandRequired)
                    .subcommand(SubCommand::with_name("init")
                                .about("Downloads and imports the set of ALL Network Rail schedules. Run to initialise the database.")
                                .arg(Arg::with_name("limit-toc")
                                     .short("t")
                                     .long("limit-toc")
                                     .value_name("TOC")
                                     .takes_value(true)
                                     .help("Limit import to a given Train Operating Company (TOC)")))
                    .subcommand(SubCommand::with_name("update")
                                .about("Downloads and imports today's schedule update file.")
                                .arg(Arg::with_name("limit-toc")
                                     .short("t")
                                     .long("limit-toc")
                                     .value_name("TOC")
                                     .takes_value(true)
                                     .help("Limit import to a given Train Operating Company (TOC)")))
                    .subcommand(SubCommand::with_name("ways")
                                .about("Makes schedule ways for unprocessed schedules."))
        )
        .get_matches();
    let mut disp = fern::Dispatch::new()
        .format(|out, msg, record| {
            out.finish(format_args!("{} [{} {}] {}",
                                    chrono::Local::now().format("[%Y-%m-%d %H:%M:%S]"),
                                    record.target(),
                                    record.level(),
                                    msg))
        })
        .level(log::LogLevelFilter::Info)
        .level_for("osms_db_setup", log::LogLevelFilter::Debug)
        .level_for("osms_db", log::LogLevelFilter::Debug)
        .chain(std::io::stdout());
    if let Some(f) = matches.value_of("logfile") {
        disp = disp.chain(fern::log_file(f)
                          .context(err_msg("couldn't open log file"))?);
    }
    disp
        .apply()
        .unwrap();
    info!("osms-db-setup tool starting");
    let path = matches.value_of("config").unwrap_or("config.toml");
    let mut cli = Client::new();
    info!("Loading config from file {}...", path);
    let mut file = File::open(path)
        .context(err_msg("couldn't open config file"))?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .context(err_msg("error reading from config file"))?;
    info!("Parsing config...");
    let conf: Config = toml::de::from_str(&contents)
        .context(err_msg("invalid config"))?;
    info!("Connecting to Postgres...");
    let r2c = r2d2::Config::default();
    let tls = if conf.require_tls {
        let tls = NativeTls::new()
            .context(err_msg("couldn't initialize tls"))?;
        TlsMode::Require(Box::new(tls))
    }
    else {
        TlsMode::None
    };
    let manager = PostgresConnectionManager::new(conf.database_url, tls)
        .context(err_msg("couldn't connect to postgres"))?;
    let pool = r2d2::Pool::new(r2c, manager).context(err_msg("couldn't make db pool"))?;

    match matches.subcommand() {
        ("setup", Some(opts)) => {
            match opts.subcommand() {
                ("init", _) => {
                    info!("Initialising database types & relations...");
                    db::initialize_database(&*pool.get().unwrap())?;
                },
                ("osm", Some(opts)) => {
                    info!("Opening map data file...");
                    let mut map = OsmPbfReader::new(BufReader::new(
                        File::open(opts.value_of_os("mapdata").unwrap())
                            .context(err_msg("couldn't open map data file"))?
                    ));
                    let mut ctx = make::ImportContext::new(&mut map, &pool, conf.threads);

                    info!("Initialising database types & relations...");
                    db::initialize_database(&*pool.get().unwrap())?;
                    if opts.is_present("count") {
                        info!("Counting objects in map file...");
                        make::count(&mut ctx)?;
                    }
                    info!("Importing data from OpenStreetMap...");
                    make::nodes(&mut ctx)?;
                    make::links(&mut ctx)?;
                    make::stations(&mut ctx)?;
                    make::separate_nodes(&mut ctx)?;
                },
                ("nrod", Some(opts)) => {
                    let conn = pool.get().unwrap();
                    info!("Initialising database types & relations...");
                    db::initialize_database(&*conn)?;
                    if util::count(&*conn, "FROM corpus_entries", &[])? == 0 {
                        info!("Importing CORPUS data...");
                        let data = File::open(opts.value_of("corpus").unwrap())
                            .context(err_msg("couldn't open corpus data file"))?;
                        let data = BufReader::new(data);
                        let data = GzDecoder::new(data)
                            .context(err_msg("couldn't start gunzipping corpus data file"))?;
                        make::corpus_entries(&*conn, data)?;
                    }
                    if util::count(&*conn, "FROM naptan_entries", &[])? == 0 {
                        let data = File::open(opts.value_of("naptan").unwrap())
                            .context(err_msg("couldn't open naptan data file"))?;
                        make::naptan_entries(&*conn, data)?;
                    }
                    if util::count(&*conn, "FROM msn_entries", &[])? == 0 {
                        let data = File::open(opts.value_of("msn").unwrap())
                            .context(err_msg("couldn't open msn data file"))?;
                        let data = BufReader::new(data);
                        make::msn_entries(&*conn, data)?;
                    }
                },
                (x, _) => panic!("Invalid setup subcommand {}", x)
            }
        },
        ("schedule", Some(opts)) => {
            match opts.subcommand() {
                ("init", Some(opts)) => {
                    let conn = pool.get().unwrap();
                    info!("Downloading & importing CIF_ALL_FULL_DAILY...");
                    let data = download("https://datafeeds.networkrail.co.uk/ntrod/CifFileAuthenticate?type=CIF_ALL_FULL_DAILY&day=toc-full",
                                        &mut cli, &conf.nrod_user, &conf.nrod_pass)?;
                    let data = BufReader::new(data);
                    let data = GzDecoder::new(data)
                        .context(err_msg("couldn't start gunzipping schedule data file"))?;
                    make::apply_schedule_records(&*conn, data, opts.value_of("limit_toc"))?;
                },
                ("update", Some(opts)) => {
                    use chrono::*;
                    let conn = pool.get().unwrap();
                    let time = Utc::now();
                    let weekd = time.weekday().pred();
                    let file = match weekd {
                        Weekday::Mon => "toc-update-mon",
                        Weekday::Tue => "toc-update-tue",
                        Weekday::Wed => "toc-update-wed",
                        Weekday::Thu => "toc-update-thu",
                        Weekday::Fri => "toc-update-fri",
                        Weekday::Sat => "toc-update-sat",
                        Weekday::Sun => "toc-update-sun",
                    };
                    info!("Downloading and importing CIF_ALL_UPDATE_DAILY {}...", file);
                    let data = download(&format!("https://datafeeds.networkrail.co.uk/ntrod/CifFileAuthenticate?type=CIF_ALL_UPDATE_DAILY&day={}", file),
                                        &mut cli, &conf.nrod_user, &conf.nrod_pass)?;
                    let data = BufReader::new(data);
                    let data = GzDecoder::new(data)
                        .context(err_msg("couldn't start gunzipping schedule data file"))?;
                    make::apply_schedule_records(&*conn, data, opts.value_of("limit_toc"))?;
                },
                ("ways", _) => {
                    make::geo_process_schedules(&pool, conf.threads)?;
                },
                (x, _) => panic!("Invalid schedule subcommand {}", x)
            }
        },
        (x, _) => panic!("Invalid subcommand {}", x)
    }

    info!("Complete!");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        error!("ERROR: {}", e);
        for c in e.causes().skip(1) {
            error!("Cause: {}", c);
        }
        if e.backtrace().to_string() == "" {
            error!("Run with RUST_BACKTRACE=1 for a backtrace.");
        }
        else {
            error!("backtrace: {}", e.backtrace());
        }
    }
}
