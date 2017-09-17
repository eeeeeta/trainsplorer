extern crate stomp;
extern crate serde_json;
extern crate osms_db;
extern crate ntrod_types;
extern crate postgres;
#[macro_use] extern crate log;
extern crate clap;
extern crate env_logger;

use stomp::handler::Handler;
use stomp::session::Session;
use stomp::frame::Frame;
use stomp::subscription::AckOrNack;
use stomp::connection::*;

use ntrod_types::movements::Records;
use postgres::{Connection, TlsMode};
use clap::{Arg, App};

fn on_message(hdl: &mut LoginHandler, sess: &mut Session<LoginHandler>, fr: &Frame) -> AckOrNack {
    let st = String::from_utf8_lossy(&fr.body);
    let recs: Result<Records, _> = serde_json::from_str(&st);
    match recs {
        Ok(r) => {
            for record in r {
                match osms_db::ntrod::live::process_ntrod_event(&hdl.0, record) {
                    Err(e) => {
                        error!("Error processing: {}", e);
                    },
                    _ => {}
                }
            }
        },
        Err(e) => error!("### PARSE ERROR ###\nerr: {}\ndata: {}", e, &st)
    }

    AckOrNack::Ack
}
struct LoginHandler(Connection);
impl Handler for LoginHandler {
    fn on_connected(&mut self, sess: &mut Session<Self>, frame: &Frame) {
        info!("Connection established.");
        sess.subscription("/t/meopic/TRAIN_MVT_ALL_TOC", on_message).start().unwrap();
    }
    fn on_error(&mut self, sess: &mut Session<Self>, frame: &Frame) {
        error!("Whoops: {}", frame);
    }
    fn on_disconnected(&mut self, sess: &mut Session<Self>) {
        warn!("Disconnected.")
    }
}
fn main() {
    env_logger::init().unwrap();
    let matches = App::new("osms-nrod")
        .author("eta <http://theta.eu.org>")
        .about("Receives data from the NTROD feeds and shoves it into the database.")
        .arg(Arg::with_name("url")
             .short("l")
             .value_name("postgresql://USER@IP/DBNAME")
             .required(true)
             .takes_value(true)
             .help("Sets the database URL to use."))
        .arg(Arg::with_name("user")
             .short("u")
             .required(true)
             .takes_value(true)
             .help("Login username."))
        .arg(Arg::with_name("pwd")
             .short("p")
             .required(true)
             .takes_value(true)
             .help("Login password."))
        .get_matches();
    let url = matches.value_of("url").unwrap();
    let user = matches.value_of("user").unwrap();
    let pwd = matches.value_of("pwd").unwrap();
    let conn = Connection::connect(url, TlsMode::None).unwrap();
    let mut cli = stomp::client::<LoginHandler>();
    cli.session("54.247.175.93", 61618, LoginHandler(conn))
        .with(Credentials(user, pwd))
        .with(HeartBeat(5_000, 2_000))
        .start();
    info!("Running client...");
    cli.run()
}
