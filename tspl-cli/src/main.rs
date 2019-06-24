use clap::{Arg, App, SubCommand, AppSettings};
use tspl_fahrplan::proto::{FahrplanRequest, FahrplanRpc, ScheduleDetails};
use tspl_fahrplan::types::Schedule;
use tspl_fahrplan::download::JobType;
use tspl_proto::RpcClient;
use chrono::NaiveDate;
use uuid::Uuid;

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
                    .subcommand(SubCommand::with_name("find_schedule_on_date")
                                .about("Find schedules matching a given UID and source that run on a given date.")
                                .arg(Arg::with_name("uid")
                                     .short("u")
                                     .long("uid")
                                     .help("UID to look for.")
                                     .value_name("UID")
                                     .required(true)
                                     .takes_value(true))
                                .arg(Arg::with_name("date")
                                     .short("d")
                                     .long("date")
                                     .help("Date on which the schedule runs.")
                                     .value_name("YYYY-MM-DD")
                                     .required(true)
                                     .takes_value(true))
                                .arg(Arg::with_name("vstp")
                                     .short("v")
                                     .long("vstp")
                                     .help("Search for VSTP schedules.")))
                    .subcommand(SubCommand::with_name("details")
                                .about("Get full details of a given schedule, identified by tspl ID.")
                                .arg(Arg::with_name("tsplid")
                                     .short("t")
                                     .long("tspl-id")
                                     .help("tspl id to search for.")
                                     .value_name("UUID")
                                     .required(true)
                                     .takes_value(true)))
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
                ("details", Some(opts)) => {
                    let uuid: Uuid = opts.value_of("tsplid").unwrap().parse().unwrap();
                    println!("[+] Getting details for {}", uuid);
                    let res: Result<Result<ScheduleDetails, _>, _> = rc.request(
                        FahrplanRequest::RequestScheduleDetails(uuid)
                    );
                    println!("[+] result: {:#?}", res);
                },
                ("find_schedule_on_date", Some(opts)) => {
                    let uid = opts.value_of("uid").unwrap();
                    let date: NaiveDate = opts.value_of("date").unwrap().parse().unwrap();
                    let source = if opts.is_present("vstp") {
                        Schedule::SOURCE_VSTP
                    }
                    else {
                        Schedule::SOURCE_ITPS
                    };
                    println!("[+] Searching for schedules with UID {}, source {} and date {}", uid, source, date);
                    let res: Result<Result<Schedule, _>, _> = rc.request(
                        FahrplanRequest::FindScheduleOnDate { 
                            uid: uid.into(),
                            on_date: date.into(),
                            source
                        }
                    );
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
