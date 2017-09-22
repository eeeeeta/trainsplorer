extern crate stomp;
extern crate serde_json;
extern crate osms_db;
extern crate ntrod_types;
extern crate postgres;
#[macro_use] extern crate log;
extern crate toml;
extern crate fern;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate enum_display_derive;
extern crate tic;

use stomp::handler::Handler;
use stomp::session::Session;
use stomp::frame::Frame;
use stomp::subscription::AckOrNack;
use stomp::connection::*;
use std::env;
use std::fs::File;
use std::io::Read;
use std::fmt::Display;
use std::thread;

use ntrod_types::movements::{MvtBody, Records};
use postgres::{Connection, TlsMode};
use tic::{Interest, Receiver, Sample, Clocksource, Sender};

#[derive(Deserialize)]
pub struct Config {
    database_url: String,
    metrics_url: String,
    username: String,
    password: String,
    #[serde(default)]
    nrod_url: Option<String>,
    #[serde(default)]
    nrod_port: Option<u16>
}
#[derive(Clone, PartialEq, Eq, Hash, Display)]
pub enum Metric {
    GotMessage,
    DidMessage,
    ParseError,
    GotActivation,
    DidActivation,
    GotCancellation,
    DidCancellation,
    GotMovement,
    DidMovement,
    Other
}
fn on_message(hdl: &mut LoginHandler, _: &mut Session<LoginHandler>, fr: &Frame) -> AckOrNack {
    let start = hdl.1.counter();
    hdl.2.send(Sample::new(start, start, Metric::GotMessage)).unwrap();
    let st = String::from_utf8_lossy(&fr.body);
    let recs: Result<Records, _> = serde_json::from_str(&st);
    match recs {
        Ok(r) => {
            for record in r {
                let metric_type = match record.body {
                    MvtBody::Activation(..) => (Metric::GotActivation, Metric::DidActivation),
                    MvtBody::Cancellation(..) => (Metric::GotCancellation, Metric::DidCancellation),
                    MvtBody::Movement(..) => (Metric::GotMovement, Metric::DidMovement),
                    _ => (Metric::Other, Metric::Other)
                };
                hdl.2.send(Sample::new(start, start, metric_type.0)).unwrap();
                match osms_db::ntrod::live::process_ntrod_event(&hdl.0, record) {
                    Err(e) => {
                        error!("Error processing: {}", e);
                    },
                    _ => {
                        let stop = hdl.1.counter();
                        hdl.2.send(Sample::new(start, stop, metric_type.1)).unwrap();
                    }
                }
            }
            let stop = hdl.1.counter();
            hdl.2.send(Sample::new(start, stop, Metric::DidMessage)).unwrap();
        },
        Err(e) => {
            let stop = hdl.1.counter();
            hdl.2.send(Sample::new(start, stop, Metric::ParseError)).unwrap();
            error!("### PARSE ERROR ###\nerr: {}\ndata: {}", e, &st);
        }
    }
    AckOrNack::Ack
}
struct LoginHandler(Connection, Clocksource, Sender<Metric>);
impl Handler for LoginHandler {
    fn on_connected(&mut self, sess: &mut Session<Self>, _: &Frame) {
        info!("Connection established.");
        sess.subscription("/topic/TRAIN_MVT_ALL_TOC", on_message).start().unwrap();
    }
    fn on_error(&mut self, _: &mut Session<Self>, frame: &Frame) {
        error!("Whoops: {}", frame);
    }
    fn on_disconnected(&mut self, _: &mut Session<Self>) {
        warn!("Disconnected.")
    }
}
fn main() {
    fern::Dispatch::new()
        .format(|out, msg, record| {
            out.finish(format_args!("[{} {}] {}",
                                    record.target(),
                                    record.level(),
                                    msg))
        })
        .level(log::LogLevelFilter::Info)
        .level_for("osms_db", log::LogLevelFilter::Debug)
        .level_for("tic", log::LogLevelFilter::Debug)
        .level_for("tiny_http", log::LogLevelFilter::Debug)
        .level_for("osms_nrod", log::LogLevelFilter::Debug)
        .chain(std::io::stdout())
        .apply()
        .unwrap();
    info!("osms-nrod starting");
    let args = env::args().skip(1).collect::<Vec<_>>();
    let path = args.get(0).map(|x| x as &str).unwrap_or("config.toml");
    info!("Loading config from file {}...", path);
    let mut file = File::open(path).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    info!("Parsing config...");
    let conf: Config = toml::de::from_str(&contents).unwrap();
    info!("Initializing metrics...");
    let mut recv = Receiver::configure()
        .service(true)
        .http_listen(conf.metrics_url.clone())
        .build();

    recv.add_interest(Interest::Count(Metric::GotMessage));
    recv.add_interest(Interest::Count(Metric::DidMessage));
    recv.add_interest(Interest::Count(Metric::ParseError));
    recv.add_interest(Interest::Count(Metric::GotActivation));
    recv.add_interest(Interest::Count(Metric::DidActivation));
    recv.add_interest(Interest::Count(Metric::GotCancellation));
    recv.add_interest(Interest::Count(Metric::DidCancellation));
    recv.add_interest(Interest::Count(Metric::GotMovement));
    recv.add_interest(Interest::Count(Metric::DidMovement));

    let sender = recv.get_sender();
    let clk = recv.get_clocksource();

    thread::spawn(move || { recv.run(); });

    info!("Connecting to database...");
    let conn = Connection::connect(conf.database_url, TlsMode::None).unwrap();
    let mut cli = stomp::client::<LoginHandler>();
    let nrod_url = conf.nrod_url.as_ref().map(|x| x as &str).unwrap_or("54.247.175.93");
    cli.session(nrod_url, conf.nrod_port.unwrap_or(61618), LoginHandler(conn, clk, sender))
        .with(Credentials(&conf.username, &conf.password))
        .with(HeartBeat(5_000, 2_000))
        .start();
    info!("Running client...");
    cli.run()
}
