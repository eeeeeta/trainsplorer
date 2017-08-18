extern crate stomp;
extern crate serde_json;
extern crate ntrod_types;

use stomp::handler::Handler;
use stomp::session::Session;
use stomp::frame::Frame;
use stomp::subscription::AckOrNack;
use stomp::connection::*;

use ntrod_types::movements::Records;
static TOKEN: &str = include_str!("../access_token");

fn on_message(hdl: &mut LoginHandler, sess: &mut Session<LoginHandler>, fr: &Frame) -> AckOrNack {
    let st = String::from_utf8_lossy(&fr.body);
    let recs: Result<Records, _> = serde_json::from_str(&st);
    match recs {
        Ok(r) => println!("recs: {:#?}", r),
        Err(e) => println!("### ERROR ###\nerr: {}\ndata: {}", e, &st)
    }

    AckOrNack::Ack
}
struct LoginHandler;
impl Handler for LoginHandler {
    fn on_connected(&mut self, sess: &mut Session<Self>, frame: &Frame) {
        println!("Subscription established.");
        sess.subscription("/topic/TRAIN_MVT_ALL_TOC", on_message).start().unwrap();
    }
    fn on_error(&mut self, sess: &mut Session<Self>, frame: &Frame) {
        println!("Whoops: {}", frame);
    }
}
fn main() {
    let mut cli = stomp::client::<LoginHandler>();
    cli.session("54.247.175.93", 61618, LoginHandler)
        .with(Credentials("eeeeeta@users.noreply.github.com", TOKEN))
        .with(HeartBeat(5_000, 2_000))
        .start();
    println!("Running client...");
    cli.run()
}
