extern crate osmpbfreader;
#[macro_use] extern crate error_chain;
extern crate osm_signal;

static OSM_DATA_LOCATION: &str = "greater-london-latest.osm.pbf";
static ACCESS_TOKEN: &str = "[REDACTED]";
static HUXLEY_URL: &str = "https://huxley.apphb.com";

use std::io::prelude::*;
use std::io::BufReader;
use std::fs::File;
use std::collections::{HashSet, HashMap};
use osmpbfreader::OsmPbfReader;
use osmpbfreader::objects::{Node, Way, OsmObj};
use osm_signal::*;
mod errors {
    error_chain! {
        links {
            Rail(::osm_signal::errors::RailError, ::osm_signal::errors::RailErrorKind);
        }
        foreign_links {
            Io(::std::io::Error);
            Osm(::osmpbfreader::error::Error);
        }
    }
}
use errors::*;

pub struct LevelCrossing {
    crossing: Node,
    highway: Option<Way>,
    railway: Option<Way>
}
fn rail() -> Result<()> {
    let mut cli = RailClient::new(ACCESS_TOKEN, HUXLEY_URL)?;
    let mut servs = HashMap::new();
    let sb = cli.station_board("CLJ", 90)?;
    for serv in sb.train_services {
        if let Some(id) = serv.id {
            if let Some(rsid) = serv.rsid {
                servs.insert(rsid, id);
            }
        }
    }
    println!("[+] {} train services", servs.len());
    for (rsid, sid) in servs {
        let serv = cli.service(&sid)?;
        println!("{:#?}", serv);
    }
    Ok(())
}
fn osm() -> Result<()> {
    let file = File::open(OSM_DATA_LOCATION)?;
    let buf_reader = BufReader::new(file);
    let mut pbf = OsmPbfReader::new(buf_reader);
    println!("[+] Finding level crossings");
    let mut crossings: HashMap<Node, LevelCrossing> = HashMap::new();
    for obj in pbf.par_iter() {
        let obj = obj?;
        if obj.is_node() && obj.tags().contains("public_transport", "stop_position") {
            if let OsmObj::Node(obj) = obj {
                crossings.insert(obj.clone(), LevelCrossing {
                    crossing: obj,
                    highway: None,
                    railway: None
                });
            }
        }
    }
    println!("[+] Found {} level crossings", crossings.len());
    pbf.rewind()?;
    println!("[+] Finding ways containing level crossings");
    for obj in pbf.par_iter() {
        let obj = obj?;
        if let OsmObj::Way(obj) = obj {
            if obj.tags.get("railway").is_some() || obj.tags.get("highway").is_some() {
                for (k, v) in crossings.iter_mut() {
                    if obj.nodes.contains(&k.id) {
                        if obj.tags.get("railway").is_some() {
                            println!("[+] Found railway: {:?}", obj.tags.get("name"));
                            if v.railway.is_some() {
                                println!("[+] DUP! Already-existing railway: {:?}", v.railway.as_ref().unwrap().tags.get("name"));
                                break;
                            }
                            v.railway = Some(obj.clone());
                        }
                        else {
                            println!("[+] Found highway: {:?}", obj.tags.get("name"));
                            if v.highway.is_some() {
                                println!("[+] DUP! Already-existing highway: {:?}", v.highway.as_ref().unwrap().tags.get("name"));
                                break;
                            }
                            v.highway = Some(obj.clone());
                        }
                    }
                }
            }
        }
    }
    println!("[+] Eliminating level crossings without ways");
    crossings.retain(|_, c| c.highway.is_some() && c.railway.is_some());
    println!("[+] Found {} complete level crossings", crossings.len());
    Ok(())
}
quick_main!(osm);
