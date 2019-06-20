use clap::{Arg, App, SubCommand, AppSettings};
use tspl_fahrplan::proto::{FahrplanRequest, FahrplanRpc};
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
                },
                _ => unreachable!()
            }
        },
        _ => unreachable!()
    }
}
