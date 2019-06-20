use clap::{Arg, App, SubCommand, AppSettings};
use tspl_fahrplan::proto::{FahrplanRequest, FahrplanRpc};
use tspl_fahrplan::types::Schedule;
use tspl_fahrplan::download::JobType;
use tspl_proto::RpcClient;

fn main() {
    let matches = App::new("tspl-cli")
        .version(env!("CARGO_PKG_VERSION"))
        .author("eta <hi@theta.eu.org>")
        .about("Sends commands to trainsplorer microservices.")
        .setting(AppSettings::SubcommandRequired)
        .arg(Arg::with_name("address")
             .short("a")
             .long("address")
             .value_name("IP:PORT")
             .help("Address of the microservice to connect to.")
             .takes_value(true)
             .required(true))
        .subcommand(SubCommand::with_name("fahrplan")
                    .about("tspl-fahrplan commands")
                    .setting(AppSettings::SubcommandRequired)
                    .subcommand(SubCommand::with_name("ping")
                                .about("Tests the connection."))
                    .subcommand(SubCommand::with_name("recover")
                                .about("Recover after a few missed schedule updates."))
                    .subcommand(SubCommand::with_name("init")
                                .about("Initialize the schedule database."))
                    .subcommand(SubCommand::with_name("search_uid")
                                .about("Find all schedules with a given NROD `uid`.")
                                .arg(Arg::with_name("uid")
                                     .short("u")
                                     .long("uid")
                                     .help("UID to look for.")
                                     .value_name("UID")
                                     .required(true)
                                     .takes_value(true)))

                    )
        .get_matches();
    println!("[+] tspl-cli");
    match matches.subcommand() {
        ("fahrplan", Some(opts)) => {
            println!("[+] Initializing fahrplan RPC");
            let dial_url = matches.value_of("address").unwrap();
            let mut rc: RpcClient<FahrplanRpc> = RpcClient::new(&dial_url).unwrap();
            match opts.subcommand() {
                ("ping", _) => {
                    println!("[+] Sending ping");
                    let res: Result<Result<String, _>, _> = rc.request(FahrplanRequest::Ping);
                    println!("[+] result: {:?}", res);
                },
                ("search_uid", Some(opts)) => {
                    let uid = opts.value_of("uid").unwrap();
                    println!("[+] Searching for schedules with UID {}", uid);
                    let res: Result<Result<Vec<Schedule>, _>, _> = rc.request(FahrplanRequest::FindSchedulesWithUid { uid: uid.into() });
                    println!("[+] result: {:#?}", res);
                },
                (x @ "init", _) | (x @ "recover", _) => {
                    let jt = match x {
                        "init" => JobType::Init,
                        "recover" => JobType::Recover,
                        _ => unreachable!()
                    };
                    println!("[+] Queuing schedule job: {:?}", jt);
                    let res: Result<Result<(), _>, _> = rc.request(FahrplanRequest::QueueUpdateJob(jt));
                    println!("[+] result: {:?}", res);
                }
                _ => unreachable!()
            }
        },
        _ => unreachable!()
    }
}
