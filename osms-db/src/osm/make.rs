use super::types::*;
use db::{GenericConnection, DbType, InsertableDbType};
use postgis::ewkb::{Point, Polygon};
use geo;
use util::*;
use errors::*;
use std::collections::{HashMap, HashSet};

pub fn make_crossings<T: GenericConnection>(conn: &T) -> Result<()> {
    debug!("make_crossings: running...");
    let trans = conn.transaction()?;
    let mut processed_osm_ids = HashSet::new();
    let mut changed = 0;
    for row in &trans.query("SELECT osm_id, way, ST_Buffer(way::geography, 20), name FROM planet_osm_point WHERE railway = 'level_crossing'", &[])? {
        let (osm_id, way, area, name): (i64, Point, Polygon, Option<String>)
            = (row.get(0), row.get(1), row.get(2), row.get(3));
        if processed_osm_ids.insert(osm_id) {
            let (node_id, _) = Node::new_at_point(&trans, way.clone())?;
            let mut other_node_ids = vec![];
            for row in &trans.query("SELECT osm_id FROM planet_osm_point WHERE ST_Intersects(way, $1)",
                                    &[&area])? {
                let osm_id: i64 = row.get(0);
                if processed_osm_ids.insert(osm_id) {
                    other_node_ids.push(Node::new_at_point(&trans, way.clone())?.0);
                }
            }
            let lxing = Crossing { node_id, name, other_node_ids, area };
            lxing.insert_self(&trans)?;
            changed += 1;
            if (changed % 100) == 0 {
                debug!("make_crossings: made {} crossings", changed);
            }
        }
    }
    trans.commit()?;
    Ok(())
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
            let cp = geo_pt_to_postgis(geocp);

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
