extern crate postgres;
#[macro_use] extern crate rouille;
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
extern crate fern;
extern crate titlecase;
extern crate handlebars;
extern crate config as cfg;
#[macro_use] extern crate log;

pub mod pool;
pub mod config;
pub mod tmpl;
pub mod templates;
#[macro_use] pub mod sctx;
pub mod qb;
pub mod schedules;
pub mod schedule;
pub mod mapping;
pub mod movements;

pub type Result<T> = ::std::result::Result<T, failure::Error>;

fn main() {
    fern::Dispatch::new()
        .format(|out, msg, record| {
            out.finish(format_args!("{} [{} {}] {}",
                                    chrono::Local::now().format("[%Y-%m-%d %H:%M:%S]"),
                                    record.target(),
                                    record.level(),
                                    msg))
        })
        .level(log::LevelFilter::Info)
        .chain(std::io::stdout())
        .apply()
        .unwrap();
    info!("osms-web starting");
    info!("Loading configuration");
    let cfg = config::Config::load().unwrap();
    info!("Connecting to database");
    let db = pool::attach_db(&cfg.database_url).unwrap();
    info!("Initializing Handlebars templating engine");
    let hbs = tmpl::handlebars_init().unwrap();
    info!("Starting server on '{}'...", cfg.listen);
    let sctx = sctx::ServerContext::new(hbs, db);
    rouille::start_server(&cfg.listen, move |req| {
        sctx::ServerContext::handle(sctx.clone(), req)
    });
}
