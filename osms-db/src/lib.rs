#[macro_use] extern crate error_chain;
extern crate postgres;
extern crate postgis;
#[macro_use] extern crate log;
extern crate ntrod_types;
extern crate chrono;
extern crate chrono_tz;
extern crate serde_json;
extern crate geo;

use std::collections::{HashSet, HashMap};
use postgres::GenericConnection;
use postgres::types::ToSql;
use postgis::ewkb::{Point, LineString, Polygon};
use ntrod_types::reference::{CorpusData, CorpusEntry};
use ntrod_types::{cif, schedule};
use std::io::{BufRead, Read};

pub mod errors {
    error_chain! {
        foreign_links {
            Io(::std::io::Error);
            Postgres(::postgres::error::Error);
            Serde(::serde_json::Error);
        }
    }
}

pub mod types;
use types::*;
use errors::*;

pub fn count<T: GenericConnection>(conn: &T, details: &str, args: &[&ToSql]) -> Result<i64> {
    Ok(conn.query(&format!("SELECT COUNT(*) {}", details), args)?.into_iter()
        .nth(0)
        .ok_or("Count query failed")?
        .get(0))
}
pub fn make_stations<T: GenericConnection>(conn: &T) -> Result<()> {
    use geo::algorithm::closest_point::ClosestPoint;
    let trans = conn.transaction()?;
    let mut areas: HashMap<String, (Polygon, Point)> = HashMap::new();
    for row in &trans.query(
        "SELECT ref, way, ST_Centroid(way)
         FROM planet_osm_polygon
         WHERE railway = 'station' AND ref IS NOT NULL", &[])? {

        areas.insert(row.get(0), (row.get(1), row.get(2)));
    }
    for row in &trans.query(
        "SELECT ref, ST_Buffer(way::geography, 50)::geometry, way
         FROM planet_osm_point
         WHERE railway = 'station' AND ref IS NOT NULL", &[])? {

        areas.insert(row.get(0), (row.get(1), row.get(2)));
    }
    debug!("make_stations: {} stations to process", areas.len());
    for (nr_ref, (poly, point)) in areas {
        let pt = geo::Point::from_postgis(&point);
        let (node, _) = Node::new_at_point(&trans, point.clone())?;
        let links = Link::from_select(&trans, "WHERE ST_Intersects(way, $1)", &[&poly])?;
        let trigd = links.len() != 0;
        for link in links {
            debug!("making new point for station {}", nr_ref);
            let geoway = geo::LineString::from_postgis(&link.way);

            let geocp = geoway.closest_point(&pt).ok_or("closest_point() returned None")?;
            let cp = types::geo_pt_to_postgis(geocp);

            let (end, trigd) = Node::new_at_point(&trans, cp)?;
            if !trigd {
                let links = Link::from_select(&trans, "WHERE p1 = $1 OR p2 = $1", &[&end])?;
                if links.len() != 0 {
                    bail!("Point ({},{}) didn't get connected to anything.",
                          geocp.lat(), geocp.lng());
                }
            }
            let connection = geo::LineString(vec![pt, geocp]);
            let link = Link {
                p1: node,
                p2: end,
                distance: 0.0,
                way: geo_ls_to_postgis(connection)
            };
            link.insert(&trans)?;
        }
        if !trigd {
            warn!("*** Station {} didn't connect to anything!", nr_ref);
        }
        Station::insert(&trans, &nr_ref, node, poly)?;
    }
    trans.commit()?;
    Ok(())
}
pub fn navigate_cached<T: GenericConnection>(conn: &T, from: &str, to: &str) -> Result<StationPath> {
    let paths = StationPath::from_select(conn, "WHERE s1 = $1 AND s2 = $2", &[&from, &to])?;
    if paths.len() > 0 {
        debug!("navigate_cached: returning memoized path from {} to {}", from, to);
        return Ok(paths.into_iter().nth(0).unwrap());
    }
    let path = navigate(conn, from, to)?;
    debug!("navigate_cached: memoizing path");
    path.insert_self(conn)?;
    Ok(path)
}
pub fn navigate<T: GenericConnection>(conn: &T, from: &str, to: &str) -> Result<StationPath> {
    // Create a transaction: we don't actually want to modify the database here.
    // This transaction will be reverted when we return.
    let trans = conn.transaction()?;

    let starting_node = Station::from_select(&trans, "WHERE nr_ref = $1", &[&from])?.into_iter()
        .nth(0).ok_or("Starting station does not exist.")?.point;

    let goal_node = Station::from_select(&trans, "WHERE nr_ref = $1", &[&to])?.into_iter()
        .nth(0).ok_or("Finishing station does not exist.")?.point;

    trans.execute("UPDATE nodes SET distance = 0 WHERE id = $1", &[&starting_node])?;

    debug!("navigate: navigating from {} ({}) to {} ({})",
           from, starting_node, to, goal_node);

    let mut cur = Node::from_select(&trans, "WHERE id = $1", &[&starting_node])
        ?.into_iter().nth(0)
        .ok_or("Starting node does not exist.")?;
    let dest = Node::from_select(&trans, "WHERE id = $1 AND graph_part = $2",
                                 &[&goal_node, &cur.graph_part])
        ?.into_iter().nth(0)
        .ok_or(
            "Finishing node does not exist, or is not in the same graph part as the starting node."
        )?;

    let mut considered = 0;
    let mut updated = 0;

    'outer: loop {
        if cur.distance == ::std::f32::INFINITY {
            error!("navigate: node {}'s distance = inf!", cur.id);
            bail!("Current node distance = inf, something has gone seriously wrong...");
        }
        trace!("considering node {} with dist {} ({}c/{}u)", cur.id, cur.distance, considered, updated);

        let links = Link::from_select(&trans, "WHERE p1 = $1 OR p2 = $1", &[&cur.id])?;
        for link in links {
            let tent_dist = link.distance + cur.distance;
            let other_end = if link.p1 == cur.id { link.p2 } else { link.p1 };
            for row in &trans.query(
                "UPDATE nodes
                 SET distance = $1
                 WHERE id = $2 AND visited = false AND distance > $1
                 RETURNING id", &[&tent_dist, &other_end])? {

                let id: i32 = row.get(0);
                updated += 1;
                trans.execute(
                    "UPDATE nodes
                     SET parent = $1, parent_geom = $2
                     WHERE id = $3", &[&cur.id, &link.way, &id])?;
                if id == dest.id {
                    break 'outer;
                }
            }
        }
        trans.execute("UPDATE nodes SET visited = true WHERE id = $1", &[&cur.id])?;
        considered += 1;
        if (considered % 1000) == 0 {
            debug!("navigate: considered {} nodes ({} updated)", considered, updated);
        }
        let next = Node::from_select(&trans,
                                     "WHERE visited = false AND graph_part = $1
                                      ORDER BY distance ASC
                                      LIMIT 1", &[&cur.graph_part])?;
        for node in next {
            cur = node;
            continue 'outer;
        }
        error!("navigate: no path found, probably an issue with the db");
        bail!("No path found!");
    }
    let mut nodes = vec![];
    let mut path = vec![];
    let mut cur_node = Node::from_select(conn,
                                         "WHERE id = $1 AND graph_part = $2",
                                         &[&goal_node, &cur.graph_part])
        ?.into_iter().nth(0).unwrap();
    loop {
        nodes.insert(0, cur_node.id);
        if cur_node.parent.is_none() {
            break;
        }
        if let Some(ref geom) = cur_node.parent_geom {
            let geom: LineString = conn.query(
                "SELECT
                 CASE WHEN ST_Intersects(ST_EndPoint($1), $2)
                      THEN $1
                      ELSE ST_Reverse($1)
                 END",
                &[&geom, &cur_node.location])
                ?.into_iter().nth(0).unwrap().get(0);
            path.insert(0, geom.clone())
        }
        let mut vec = Node::from_select(conn, "WHERE id = $1", &[&cur_node.parent.unwrap()])?;
        cur_node = vec.remove(0);
    }
    let path: LineString = conn.query("SELECT ST_MakeLine(CAST($1 AS geometry[]))", &[&path])
        ?.into_iter().nth(0).unwrap().get(0);
    debug!("navigate: completed");
    Ok(StationPath { s1: from.to_string(), s2: to.to_string(), way: path, nodes })
}
pub fn make_nodes<T: GenericConnection>(conn: &T) -> Result<()> {
    debug!("make_nodes: making nodes from OSM data...");
    let trans = conn.transaction()?;
    let mut compl = 0;
    for row in &trans.query("SELECT ST_StartPoint(way), ST_EndPoint(way)
                            FROM planet_osm_line WHERE railway IS NOT NULL", &[])? {
        Node::insert(&trans, row.get(0))?;
        Node::insert(&trans, row.get(1))?;
        compl += 1;
        if (compl % 1000) == 0 {
            debug!("make_nodes: completed {} rows", compl);
        }
    }
    trans.commit()?;
    debug!("make_nodes: complete!");
    Ok(())
}
pub fn make_links<T: GenericConnection>(conn: &T) -> Result<()> {
    debug!("make_links: making links from OSM data...");
    let trans = conn.transaction()?;
    let mut compl = 0;
    for node in Node::from_select(&trans, "", &[])? {
        for row in &trans.query("SELECT way, CAST(ST_Length(way::geography, false) AS REAL), id
                                FROM planet_osm_line
                                INNER JOIN nodes ON ST_EndPoint(planet_osm_line.way) = nodes.location
                                WHERE railway IS NOT NULL AND ST_Intersects(ST_StartPoint(way), $1)",
                                &[&node.location])? {
            let link = Link { p1: node.id, p2: row.get(2), way: row.get(0), distance: row.get(1) };
            link.insert(&trans)?;
        }
        compl += 1;
        if (compl % 1000) == 0 {
            debug!("make_links: completed {} rows", compl);
        }
    }
    trans.commit()?;
    debug!("make_links: complete!");
    Ok(())
}
pub fn the_great_connectifier<T: GenericConnection>(conn: &T) -> Result<()> {
    debug!("the_great_connectifier: running...");
    let trans = conn.transaction()?;
    let mut compl = 0;
    for n1 in Node::from_select(&trans, "", &[])? {
        let _ = Node::new_at_point(&trans, n1.location);
        compl += 1;
        if (compl % 1000) == 0 {
            debug!("the_great_connectifier: completed {} rows", compl);
        }
    }
    trans.commit()?;
    debug!("the_great_connectifier: complete!");
    Ok(())
}
pub fn separate_nodes<T: GenericConnection>(conn: &T) -> Result<()> {
    debug!("separate_nodes: running...");
    let trans = conn.transaction()?;
    let mut cur_graph_part = 1;
    loop {
        let vec = Node::from_select(&trans, "WHERE graph_part = 0 LIMIT 1", &[])?;
        if vec.len() == 0 {
            break;
        }
        let mut nodes_touched = 0;
        for node in vec {
            let mut part_of_this = HashSet::new();
            part_of_this.insert(node.id);
            let mut current_roots = HashSet::new();
            current_roots.insert(node.id);
            loop {
                if current_roots.len() == 0 {
                    nodes_touched = part_of_this.len();
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
            trans.execute("UPDATE nodes SET graph_part = $1 WHERE id = ANY($2)",
                          &[&cur_graph_part, &part_of_this])?;
        }
        if nodes_touched > 10 {
            debug!("separate_nodes: finished processing graph part {}", cur_graph_part);
        }
        cur_graph_part += 1;
    }
    trans.commit()?;
    debug!("separate_nodes: separated graph into {} parts", cur_graph_part);
    Ok(())
}
pub fn make_schedule_ways<T: GenericConnection>(conn: &T) -> Result<()> {
    debug!("make_schedule_ways: getting schedules...");
    let scheds = Schedule::from_select(conn, "WHERE cardinality(ways) = 0", &[])?;
    debug!("make_schedule_ways: {} schedules to update", scheds.len());
    for sched in scheds {
        let trans = conn.transaction()?;
        sched.make_ways(&trans)?;
        trans.commit()?;
    }
    debug!("make_schedule_ways: complete!");
    Ok(())
}
pub fn apply_schedule_records<T: GenericConnection, R: BufRead>(conn: &T, rdr: R) -> Result<()> {
    debug!("apply_schedule_records: running...");
    let mut inserted = 0;
    let trans = conn.transaction()?;
    for line in rdr.lines() {
        let line = line?;
        let rec: ::std::result::Result<schedule::Record, _> = serde_json::from_str(&line);
        let rec = match rec {
            Ok(r) => r,
            Err(e) => {
                warn!("apply_schedule_records: error parsing: {}", e);
                debug!("apply_schedule_records: line was: {}", line);
                continue;
            }
        };
        match rec {
            schedule::Record::Schedule(rec) => {
                Schedule::apply_rec(&trans, rec)?;
                inserted += 1;
            },
            schedule::Record::Timetable(rec) => {
                debug!("apply_schedule_records: this is a {}-type timetable from {} (ts: {})",
                       rec.classification, rec.owner, rec.timestamp);
            },
            _ => {}
        }
    }
    trans.commit()?;
    debug!("apply_schedule_record: applied {} entries", inserted);
    Ok(())
}
pub fn import_corpus<T: GenericConnection, R: Read>(conn: &T, rdr: R) -> Result<()> {
    debug!("import_corpus: loading data from file...");
    let data: CorpusData = serde_json::from_reader(rdr)?;
    debug!("import_corpus: inserting data into database...");
    let mut inserted = 0;
    let trans = conn.transaction()?;
    for ent in data.tiploc_data {
        if ent.contains_data() {
            ent.insert_self(&trans)?;
            inserted += 1;
        }
    }
    trans.commit()?;
    debug!("import_corpus: inserted {} entries", inserted);
    Ok(())
}
pub fn initialize_database<T: GenericConnection>(conn: &T) -> Result<()> {
    debug!("initialize_database: making types...");
    conn.execute(schedule::Days::create_type(), &[])?;
    conn.execute(cif::StpIndicator::create_type(), &[])?;
    debug!("initialize_database: making tables...");
    Node::make_table(conn)?;
    Link::make_table(conn)?;
    Station::make_table(conn)?;
    StationPath::make_table(conn)?;
    Schedule::make_table(conn)?;
    ScheduleLocation::make_table(conn)?;
    Train::make_table(conn)?;
    ScheduleWay::make_table(conn)?;
    CorpusEntry::make_table(conn)?;
    let mut changed = false;
    let mut nodes = count(conn, "FROM nodes", &[])?;
    if nodes == 0 {
        make_nodes(conn)?;
        nodes = count(conn, "FROM nodes", &[])?;
        changed = true;
        debug!("initialize_database: {} nodes after node creation", nodes);
    }
    let mut links = count(conn, "FROM links", &[])?;
    if links == 0 {
        make_links(conn)?;
        links = count(conn, "FROM links", &[])?;
        changed = true;
        debug!("initialize_database: {} links after link creation", nodes);
    }
    let mut stations = count(conn, "FROM stations", &[])?;
    if stations == 0 {
        make_stations(conn)?;
        stations = count(conn, "FROM stations", &[])?;
        changed = true;
        debug!("initialize_database: {} stations after station creation", nodes);
    }
    if changed {
        debug!("initialize_database: changes occurred, running connectifier...");
        the_great_connectifier(conn)?;
        debug!("initialize_database: {} nodes, {} links after connectification", nodes, links);
    }
    let unclassified = count(conn, "FROM nodes WHERE graph_part = 0", &[])?;
    if unclassified != 0 || changed {
        debug!("initialize_database: running node separator...");
        separate_nodes(conn)?;
    }
    debug!("initialize_database: database OK (nodes = {}, links = {}, stations = {})",
           nodes, links, stations);
    Ok(())
}
