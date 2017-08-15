#[macro_use] extern crate error_chain;
extern crate national_rail_departures as nrd;
extern crate postgres;
extern crate indicatif;
extern crate postgis;

static ACCESS_TOKEN: &str = include_str!("../access_token");
static DATABASE_URL: &str = "postgresql://eeeeeta@127.0.0.1/osm";
static HUXLEY_URL: &str = "https://huxley.apphb.com";

use std::collections::{HashSet, HashMap};
use postgres::{Connection, GenericConnection, TlsMode};
use postgres::types::ToSql;
use postgis::ewkb::{Point, LineString, Polygon};
use nrd::*;
use indicatif::ProgressBar;

mod errors {
    error_chain! {
        links {
            Rail(::nrd::errors::RailError, ::nrd::errors::RailErrorKind);
        }
        foreign_links {
            Io(::std::io::Error);
            Postgres(::postgres::error::Error);
            PostgresConnect(::postgres::error::ConnectError);
        }
    }
}
mod types;
use types::*;
use errors::*;
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

fn count<T: GenericConnection>(conn: &T, details: &str, args: &[&ToSql]) -> Result<i64> {
    Ok(conn.query(&format!("SELECT COUNT(*) {}", details), args)?.into_iter()
        .nth(0)
        .ok_or("Count query failed")?
        .get(0))
}
fn new_node_at_point<T: GenericConnection>(conn: &T, point: Point) -> Result<i32> {
    let trans = conn.transaction()?;
    let links = Link::from_select(&trans, "WHERE ST_Intersects(way, $1) AND NOT ST_Intersects(ST_EndPoint(way), $1) AND NOT ST_Intersects(ST_StartPoint(way), $1)", &[&point])?;
    if links.len() > 1 {
        bail!("There are >1 links containing that point! Something is broken.");
    }
    let node = Node::insert(&trans, point.clone())?;
    for link in links {
        //println!("[+] Splitting link {} <-> {}", link.p1, link.p2);
        for row in &trans.query("SELECT ST_GeometryN(ST_Split($1, $2), 1), ST_GeometryN(ST_Split($1, $2), 2), CAST(ST_Length(ST_GeometryN(ST_Split($1, $2), 1)) AS REAL), CAST(ST_Length(ST_GeometryN(ST_Split($1, $2), 2)) AS REAL)", &[&link.way, &point])? {
            let (first, last): (LineString, LineString) = (row.get(0), row.get(1));
            let (df, dl): (f32, f32) = (row.get(2), row.get(3));
            trans.execute("UPDATE links SET p2 = $1, way = $2, distance = $3 WHERE p1 = $4 AND p2 = $5",
                         &[&node, &first, &df, &link.p1, &link.p2])?;
            let new = Link {
                p1: node,
                p2: link.p2,
                distance: dl,
                way: last
            };
            //println!("[+] New setup: {} <-> {} <-> {}", link.p1, node, link.p2);
            new.insert(&trans)?;
        }
    }
    trans.commit()?;
    Ok(node)
}
fn make_stations<T: GenericConnection>(conn: &T) -> Result<()> {
    let trans = conn.transaction()?;
    let mut areas: HashMap<String, (Polygon, Point)> = HashMap::new();
    for row in &trans.query("SELECT ref, way, ST_Centroid(way) FROM planet_osm_polygon WHERE railway = 'station' AND ref IS NOT NULL", &[])? {
        areas.insert(row.get(0), (row.get(1), row.get(2)));
    }
    for row in &trans.query("SELECT ref, ST_Buffer(way::geography, 50)::geometry, way FROM planet_osm_point WHERE railway = 'station' AND ref IS NOT NULL", &[])? {
        areas.insert(row.get(0), (row.get(1), row.get(2)));
    }
    println!("[+] {} stations to process", areas.len());
    let bar = ProgressBar::new(areas.len() as _);
    for (nr_ref, (poly, point)) in areas {
        let node = new_node_at_point(&trans, point.clone())?;
        for row in &trans.query("SELECT ST_ShortestLine($1, way), ST_EndPoint(ST_ShortestLine($1, way)) FROM planet_osm_line WHERE railway = 'rail' AND ST_Intersects(way, $2)",
                                &[&point, &poly])? {
            let (way, end): (LineString, Point) = (row.get(0), row.get(1));
            let end = new_node_at_point(&trans, end)?;
            let link = Link {
                p1: node,
                p2: end,
                distance: 0.0,
                way: way
            };
            link.insert(&trans)?;
        }
        Station::insert(&trans, &nr_ref, node, poly)?;
        bar.inc(1);
    }
    bar.finish();
    trans.commit()?;
    Ok(())
}
fn osm() -> Result<()> {
    let conn = Connection::connect(DATABASE_URL, TlsMode::None)?;
    println!("[+] Creating tables...");
    Node::make_table(&conn)?;
    Link::make_table(&conn)?;
    Station::make_table(&conn)?;
    let cnt = count(&conn, "FROM planet_osm_line WHERE railway IS NOT NULL", &[])?;
    println!("[+] {} ways in database", cnt);
    let mut nodes = count(&conn, "FROM nodes", &[])?;
    if nodes == 0 {
        println!("[+] Making nodes...");
        let bar = ProgressBar::new(cnt as _);
        let trans = conn.transaction()?;
        for row in &trans.query("SELECT ST_StartPoint(way), ST_EndPoint(way) FROM planet_osm_line WHERE railway IS NOT NULL", &[])? {
            Node::insert(&trans, row.get(0))?;
            Node::insert(&trans, row.get(1))?;
            bar.inc(1);
        }
        trans.commit()?;
        bar.finish();
        nodes = count(&conn, "FROM nodes", &[])?;
    }
    println!("[+] {} nodes in database", nodes);
    let mut links = count(&conn, "FROM links", &[])?;
    if links == 0 {
        println!("[+] Forming links...");
        let bar = ProgressBar::new(nodes as _);
        let trans = conn.transaction()?;
        for node in Node::from_select(&trans, "", &[])? {
            for row in &trans.query("SELECT way, CAST(ST_Length(way) AS REAL), id FROM planet_osm_line INNER JOIN nodes ON ST_EndPoint(planet_osm_line.way) = nodes.location WHERE railway IS NOT NULL AND ST_Intersects(ST_StartPoint(way), $1)", &[&node.location])? {
                let link = Link { p1: node.id, p2: row.get(2), way: row.get(0), distance: row.get(1) };
                link.insert(&trans)?;
            }
            bar.inc(1);
        }
        trans.commit()?;
        bar.finish();
        links = count(&conn, "FROM links", &[])?;
    }
    println!("[+] {} links in database", links);
    let mut stations = count(&conn, "FROM stations", &[])?;
    if stations == 0 {
        println!("[+] Processing stations...");
        make_stations(&conn)?;
        stations = count(&conn, "FROM stations", &[])?;
    }
    println!("[+] {} stations in database", stations);
    let unclassed_nodes = count(&conn, "FROM nodes WHERE graph_part = 0", &[])?;
    let mut cur_graph_part = 1;
    if unclassed_nodes > 0 {
        println!("[+] Separating nodes...");
        let bar = ProgressBar::new_spinner();
        let trans = conn.transaction()?;
        let mut total = 0;
        loop {
            let vec = Node::from_select(&trans, "WHERE graph_part = 0 LIMIT 1", &[])?;
            if vec.len() == 0 {
                break;
            }
            bar.set_position(0);
            for node in vec {
                let mut part_of_this = HashSet::new();
                part_of_this.insert(node.id);
                let mut current_roots = HashSet::new();
                current_roots.insert(node.id);
                loop {
                    bar.set_message(&format!("Considering graph part {} (this = {}, total = {}/{})", cur_graph_part, part_of_this.len(), total+part_of_this.len(), nodes));
                    if current_roots.len() == 0 {
                        total += part_of_this.len();
                        break;
                    }
                    for root in ::std::mem::replace(&mut current_roots, HashSet::new()) {
                        for link in Link::from_select(&trans, "WHERE p1 = $1 OR p2 = $1", &[&root])? {
                            let other_end = if link.p1 == root { link.p2 } else { link.p1 };
                            if other_end != root && part_of_this.insert(other_end) {
                                current_roots.insert(other_end);
                            }
                        }
                    }
                }
                let part_of_this = part_of_this.into_iter().collect::<Vec<_>>();
                trans.execute("UPDATE nodes SET graph_part = $1 WHERE id = ANY($2)", &[&cur_graph_part, &part_of_this])?;
            }
            cur_graph_part += 1;
        }
        trans.commit()?;
        bar.finish();
    }
    else {
        for node in Node::from_select(&conn, "ORDER BY graph_part DESC LIMIT 1", &[])? {
            cur_graph_part = node.graph_part;
        }
    }
    println!("[+] All nodes separated, {} graph parts", cur_graph_part);
    conn.execute("UPDATE nodes SET distance = 'Infinity', visited = false, parent = NULL, parent_geom = NULL", &[])?;
    let start = "CLJ";
    let end = "QRB";
    let starting_node = Station::from_select(&conn, "WHERE nr_ref = $1", &[&start])?[0].point;
    let goal_node = Station::from_select(&conn, "WHERE nr_ref = $1", &[&end])?[0].point;
    conn.execute("UPDATE nodes SET distance = 0 WHERE id = $1", &[&starting_node])?;
    let trans = conn.transaction()?;
    println!("[+] Navigating from {} to {}", starting_node, goal_node);
    let mut cur = Node::from_select(&trans, "WHERE id = $1", &[&starting_node])?.into_iter().nth(0)
        .ok_or("Starting node does not exist!")?;
    let dest = Node::from_select(&trans, "WHERE id = $1 AND graph_part = $2", &[&goal_node, &cur.graph_part])?.into_iter().nth(0)
        .ok_or("Finishing node does not exist, or is not in the same graph part as the starting node")?;
    let bar = ProgressBar::new_spinner();
    let mut distance: f32 = 0.0;
    let mut considered = 0;
    let mut updated = 0;
    'outer: loop {
        assert!(cur.distance != ::std::f32::INFINITY);
        bar.set_message(&format!("Considering node {} ({} considered, {} updated)", cur.id, considered, updated));
        let links = Link::from_select(&trans, "WHERE p1 = $1 OR p2 = $1", &[&cur.id])?;
        for link in links {
            let tent_dist = link.distance + cur.distance;
            let other_end = if link.p1 == cur.id { link.p2 } else { link.p1 };
            for row in &trans.query("UPDATE nodes SET distance = $1 WHERE id = $2 AND visited = false AND distance > $1 RETURNING id", &[&tent_dist, &other_end])? {
                let id: i32 = row.get(0);
                updated += 1;
                trans.execute("UPDATE nodes SET parent = $1, parent_geom = $2 WHERE id = $3", &[&cur.id, &link.way, &id])?;
                if id == dest.id {
                    distance = tent_dist;
                    break 'outer;
                }
            }
        }
        trans.execute("UPDATE nodes SET visited = true WHERE id = $1", &[&cur.id])?;
        considered += 1;
        let next = Node::from_select(&trans, "WHERE visited = false AND graph_part = $1 ORDER BY distance ASC LIMIT 1", &[&cur.graph_part])?;
        for node in next {
            cur = node;
            continue 'outer;
        }
        break;
    }
    trans.commit()?;
    bar.finish();
    if distance == 0.0 {
        println!("[+] It is unpossible! No path found...");
        return Ok(());
    }
    println!("[+] Djikstra's algorithm complete! Distance = {}", distance);
    println!("[+] Producing actual path...");
    let mut ret = vec![];
    let mut cur_node = Node::from_select(&conn, "WHERE id = $1 AND graph_part = $2", &[&goal_node, &cur.graph_part])?.into_iter().nth(0).unwrap();
    loop {
        ret.push(cur_node.id);
        if cur_node.parent.is_none() {
            break;
        }
        let mut vec = Node::from_select(&conn, "WHERE id = $1", &[&cur_node.parent.unwrap()])?;
        cur_node = vec.remove(0);
    }
    let ret = ret.iter().rev().collect::<Vec<_>>();
    println!("[+] Path is: {:?}", ret);
    println!(r#"
<?xml version="1.0" encoding="UTF-8"?>
<gpx
 version="1.0"
creator="GPSBabel - http://www.gpsbabel.org"
xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
xmlns="http://www.topografix.com/GPX/1/0"
xsi:schemaLocation="http://www.topografix.com/GPX/1/0 http://www.topografix.com/GPX/1/0/gpx.xsd">
<trk>
<trkseg>
"#);
    for node in ret {
        for node in Node::from_select(&conn, "WHERE id = $1", &[&node])? {
            println!(r#"<trkpt lat="{}" lon="{}" />"#, node.location.y, node.location.x);
        }
    }
    println!(r#"
</trkseg>
</trk>
</gpx>"#);
    Ok(())
}
quick_main!(rail);
