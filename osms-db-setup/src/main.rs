extern crate osms_db;
extern crate fern;
extern crate envy;
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
extern crate fallible_iterator;
extern crate serde_json;
extern crate irc;

use clap::{Arg, App, SubCommand, AppSettings};
use std::fs::File;
use std::io::{BufReader};
use r2d2_postgres::{TlsMode, PostgresConnectionManager};
use postgres::tls::native_tls::NativeTls;
use osms_db::*;
use flate2::bufread::GzDecoder;
use osmpbfreader::OsmPbfReader;
use failure::{Error, ResultExt, err_msg};
use reqwest::{Response, Client};
use reqwest::header::{Authorization, Basic};
use fallible_iterator::FallibleIterator;
use irc::client::prelude::*;
use irc::client::Client as IrcClientTrait;
use irc::client::ext::ClientExt;

pub mod make;

#[derive(Deserialize, Debug)]
pub struct Config {
    database_url: String,
    username: String,
    password: String,
    require_tls: bool,
    n_threads: usize,
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
        .arg(Arg::with_name("logfile")
             .short("l")
             .long("logfile")
             .value_name("FILE")
             .help("Path to an optional logfile to write log messages to.")
             .takes_value(true))
        .arg(Arg::with_name("ircconf")
             .short("i")
             .long("ircconf")
             .value_name("FILE")
             .help("Path to an IRC client configuration file, if you want to emit progress information to IRC."))
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
                                .about("Downloads and imports the set of ALL Network Rail schedules. Run to initialise the database."))
                    .subcommand(SubCommand::with_name("update")
                                .about("Downloads and imports today's schedule update file.")
                                .arg(Arg::with_name("retries")
                                     .short("r")
                                     .long("retries")
                                     .value_name("NUMBER")
                                     .help("Number of times to retry getting the schedule update file; defaults to 9.")
                                     .takes_value(true)))
                    .subcommand(SubCommand::with_name("recover")
                                .about("Recovers after a schedule update file is missed."))
                    .subcommand(SubCommand::with_name("listener")
                                .about("Starts a long-running listener that makes new schedule ways every time schedules are changed."))
                    .subcommand(SubCommand::with_name("ways")
                                .about("Makes schedule ways for unprocessed schedules."))
        )
        .subcommand(SubCommand::with_name("migration")
                    .about("Performs (potentially dangerous!) operations related to database migrations.")
                    .setting(AppSettings::SubcommandRequired)
                    .subcommand(SubCommand::with_name("list")
                                .about("List all the available migrations."))
                    .subcommand(SubCommand::with_name("run")
                                .about("Apply any unapplied migrations."))
                    .subcommand(SubCommand::with_name("apply")
                                .about("Apply a migration.")
                                .arg(Arg::with_name("migration")
                                     .short("m")
                                     .long("migration")
                                     .value_name("ID")
                                     .help("ID of the migration to use.")
                                     .required(true)
                                     .takes_value(true)))
                    .subcommand(SubCommand::with_name("undo")
                                .about("Undo a migration.")
                                .arg(Arg::with_name("migration")
                                     .short("m")
                                     .long("migration")
                                     .value_name("ID")
                                     .help("ID of the migration to use.")
                                     .required(true)
                                     .takes_value(true)))
                    .subcommand(SubCommand::with_name("fudge")
                                .about("Pretend as if a migration was applied, whilst doing nothing (DANGEROUS!)")
                                .arg(Arg::with_name("migration")
                                     .short("m")
                                     .long("migration")
                                     .value_name("ID")
                                     .help("ID of the migration to use.")
                                     .required(true)
                                     .takes_value(true)))
                    .subcommand(SubCommand::with_name("unfudge")
                                .about("Pretend as if a migration was undone, whilst doing nothing (DANGEROUS!)")
                                .arg(Arg::with_name("migration")
                                     .short("m")
                                     .long("migration")
                                     .value_name("ID")
                                     .help("ID of the migration to use.")
                                     .required(true)
                                     .takes_value(true)))
        )
        .get_matches();
    println!("[+] osms-db-setup tool starting");
    let mut disp = fern::Dispatch::new()
        .format(|out, msg, record| {
            out.finish(format_args!("{} [{} {}] {}",
                                    chrono::Local::now().format("[%Y-%m-%d %H:%M:%S]"),
                                    record.target(),
                                    record.level(),
                                    msg))
        })
        .level(log::LevelFilter::Info)
        .level_for("osms_db_setup", log::LevelFilter::Debug)
        .level_for("osms_db", log::LevelFilter::Debug)
        .chain(std::io::stdout());
    if let Some(f) = matches.value_of("logfile") {
        println!("[+] Logging to logfile: {}", f);
        disp = disp.chain(fern::log_file(f)
                          .context(err_msg("couldn't open log file"))?);
    }
    if let Some(i) = matches.value_of("ircconf") {
        println!("[+] Setting up IRC logging...");
        let i = IrcClient::new(i)?;
        if i.config().channels().len() == 0 {
            bail!("[!] IRC client improperly configured: no channels to send to");
        }
        i.identify()?;
        let (tx, rx) = ::std::sync::mpsc::channel::<String>();
        ::std::thread::spawn(move || {
            // let the client connect (this is a bit hacky, but hey)
            while i.list_channels().unwrap().len() == 0 {
                ::std::thread::sleep(::std::time::Duration::from_millis(100));
            }
            let chan = i.config().channels()[0];
            while let Ok(msg) = rx.recv() {
                if msg.contains("DEBUG") || msg.contains("TRACE") {
                    continue; // this is even more hacky :P
                }
                // don't log the timestamp
                let msg = msg.split(" ").skip(1).collect::<String>();
                if let Err(e) = i.send_privmsg(chan, msg) {
                    eprintln!("[!] Failed to send IRC message: {}", e);
                }
            }
        });
        disp = disp.chain(tx);
    }
    disp
        .apply()
        .unwrap();
    let mut cli = Client::new();
    info!("Loading config...");
    let conf: Config = envy::from_env()?; 
    info!("Connecting to Postgres...");
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
    let pool = r2d2::Pool::builder()
        .build(manager)
        .context(err_msg("couldn't make db pool"))?;
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
                    let mut ctx = make::ImportContext::new(&mut map, &pool, conf.n_threads);

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
                        let data = GzDecoder::new(data);
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
                (x @ "init", _) | (x @ "recover", _) => {
                    let conn = pool.get().unwrap();
                    info!("Initialising database types & relations...");
                    db::initialize_database(&*pool.get().unwrap())?;
                    info!("Downloading & importing CIF_ALL_FULL_DAILY...");
                    let data = download("https://datafeeds.networkrail.co.uk/ntrod/CifFileAuthenticate?type=CIF_ALL_FULL_DAILY&day=toc-full",
                                        &mut cli, &conf.username, &conf.password)?;
                    let data = BufReader::new(data);
                    let data = GzDecoder::new(data);
                    make::apply_schedule_records(&*conn, data)?;
                    if x == "recover" {
                        use db::DbType;

                        info!("Beginning recovery...");
                        let sf = ::osms_db::ntrod::types::ScheduleFile::from_select(&*conn, "ORDER BY metaseq DESC LIMIT 1", &[])?;
                        if let Some(sf) = sf.into_iter().nth(0) {
                            info!("Last metaseq was {}. Deleting stale schedules...", sf.metaseq);
                            conn.execute("DELETE FROM schedules WHERE file_metaseq != $1", &[&sf.metaseq])?;
                        }
                        else {
                            warn!("No schedules inserted yet!");
                        }
                    }
                },
                ("update", _) => {
                    use chrono::*;
                    let conn = pool.get().unwrap();
                    info!("Initialising database types & relations...");
                    db::initialize_database(&*pool.get().unwrap())?;
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
                    let retries = opts.value_of("retries").unwrap_or("9");
                    let retries = retries.parse::<usize>()?;
                    info!("Attempting to fetch CIF_ALL_FULL_DAILY, with {} retries.", retries);
                    let mut cur = 0;
                    let mut timeout_ms = 16000;
                    let username = &conf.username;
                    let password = &conf.password;
                    let mut do_fetch = || -> Result<(), ::failure::Error> {
                        let data = download(&format!("https://datafeeds.networkrail.co.uk/ntrod/CifFileAuthenticate?type=CIF_ALL_UPDATE_DAILY&day={}", file),
                        &mut cli, username, password)?;
                        let data = BufReader::new(data);
                        let data = GzDecoder::new(data);
                        make::apply_schedule_records(&*conn, data)
                    };
                    loop {
                        info!("Attempting to fetch {} (try {})...", file, cur+1);
                        match do_fetch() {
                            Ok(()) => {
                                info!("Successful!");
                                break;
                            },
                            Err(e) => {
                                warn!("Failed to fetch: {}", e);
                                cur += 1;
                                if cur > retries {
                                    error!("Attempt to fetch timed out!");
                                    Err(format_err!("fetch attempts timed out"))?
                                }
                                warn!("Retrying in {} ms...", timeout_ms);
                                ::std::thread::sleep(::std::time::Duration::from_millis(timeout_ms));
                                timeout_ms *= 2;
                            }
                        }
                    }
                },
                ("ways", _) => {
                    let conn = pool.get().unwrap();
                    info!("Initialising database types & relations...");
                    db::initialize_database(&*conn)?;
                    make::geo_process_schedules(&pool, conf.n_threads)?;
                },
                ("listener", _) => {
                    let conn = pool.get().unwrap();
                    info!("Initialising database types & relations...");
                    db::initialize_database(&*conn)?;
                    info!("Doing first run...");
                    make::geo_process_schedules(&pool, conf.n_threads)?;
                    info!("Listening for notifications...");
                    conn.execute("LISTEN osms_schedule_updates;", &[])?;
                    let notifs = conn.notifications();
                    let mut iter = notifs.blocking_iter();
                    while let Some(notif) = iter.next()? {
                        info!("Got notification from pid {} with payload '{}'; updating...", notif.process_id, notif.payload);
                        make::geo_process_schedules(&pool, conf.n_threads)?;
                        info!("Finished processing notification");
                    }
                },
                (x, _) => panic!("Invalid schedule subcommand {}", x)
            }
        },
        ("migration", Some(opts)) => {
            let conn = pool.get().unwrap();
            migration::initialize_migrations(&*conn)?;
            match opts.subcommand() {
                ("list", _) => {
                    use db::DbType;

                    println!("database migrations (* = applied):\n");
                    for m in migration::MIGRATIONS.iter() {
                        print!("[{}] {}", m.id, m.name);
                        if migration::MigrationEntry::from_select(&*conn, "WHERE id = $1", &[&m.id])?.len() > 0 {
                            print!(" (*)");
                        }
                        println!();
                    }
                },
                ("run", _) => {
                    db::initialize_database(&*conn)?;
                },
                (_, None) => panic!("invalid subcommand"),
                (a, Some(opts)) => {
                    let mid = opts.value_of("migration").unwrap();
                    let mid = mid.parse::<i32>()?;
                    match a {
                        x @ "apply" | x @ "undo" => {
                            if let Ok(elem) = migration::MIGRATIONS.binary_search_by_key(&mid, |m| m.id) {
                                let elem = &migration::MIGRATIONS[elem];
                                if x == "apply" {
                                    elem.up(&*conn)?;
                                }
                                else {
                                    elem.down(&*conn)?;
                                }
                            }
                            else {
                                error!("no such migration");
                                return Err(format_err!("no such migration"));
                            }
                        },
                        "fudge" => {
                            use db::InsertableDbType;

                            warn!("I hope you know what you're doing. If not, run `unfudge` after this finishes.");
                            migration::MigrationEntry {
                                id: mid,
                                timestamp: ::chrono::Utc::now().naive_utc()
                            }.insert_self(&*conn)?;
                        },
                        "unfudge" => {
                            warn!("I hope you know what you're doing.");
                            conn.execute("DELETE FROM migration_entries WHERE id = $1", &[&mid])?;
                        },
                        x => panic!("Invalid migration subcommand {}", x)
                    }
                }
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
        for c in e.iter_causes() {
            error!("Cause: {}", c);
        }
        if e.backtrace().to_string() == "" {
            error!("Run with RUST_BACKTRACE=1 for a backtrace.");
        }
        else {
            error!("backtrace: {}", e.backtrace());
        }
        ::std::process::exit(1);
    }
}
