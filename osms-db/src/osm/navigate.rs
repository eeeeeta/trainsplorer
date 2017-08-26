use super::types::*;
use db::{GenericConnection, DbType, InsertableDbType};
use errors::*;
use postgis::ewkb::LineString;

pub fn navigate_cached<T: GenericConnection>(conn: &T, from: &str, to: &str) -> Result<i32> {
    let paths = StationPath::from_select(conn, "WHERE s1 = $1 AND s2 = $2", &[&from, &to])?;
    if paths.len() > 0 {
        debug!("navigate_cached: returning memoized path from {} to {}", from, to);
        return Ok(paths.into_iter().nth(0).unwrap().id);
    }
    let path = navigate(conn, from, to)?;
    debug!("navigate_cached: memoizing path");
    Ok(path.insert_self(conn)?)
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
                                         &[&goal_node, &cur.graph_part])?.into_iter()
        .nth(0).unwrap();
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
    debug!("navigate: finding intersecting crossings...");
    let mut crossings = vec![];
    let mut crossing_locations = vec![];
    for cx in Crossing::from_select(conn, "WHERE ST_Intersects(area, $1)", &[&path])? {
        crossings.push(cx.node_id);
        for row in &trans.query("SELECT ST_Line_Locate_Point($1, ST_Centroid($2))",
                                &[&path, &cx.area])? {
            let location: f64 = row.get(0);
            crossing_locations.push(location);
        }
    }
    debug!("navigate: completed");
    Ok(StationPath {
        s1: from.to_string(), s2: to.to_string(), way: path,
        nodes, crossings, crossing_locations,
        id: -1
    })
}
