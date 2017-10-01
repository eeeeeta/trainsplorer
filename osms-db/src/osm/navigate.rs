use super::types::*;
use db::{GenericConnection, DbType, InsertableDbType};
use errors::*;
use postgis::ewkb::LineString;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, BinaryHeap};
use ordered_float::OrderedFloat;

struct LightNode {
    parent: Option<(i32, LineString)>,
    dist: f32
}
#[derive(Copy, Clone, Eq, PartialEq)]
struct State {
    cost: OrderedFloat<f32>,
    id: i32
}

impl Ord for State {
    fn cmp(&self, other: &State) -> Ordering {
        other.cost.cmp(&self.cost)
            .then_with(|| self.id.cmp(&other.id))
    }
}

impl PartialOrd for State {
    fn partial_cmp(&self, other: &State) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
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

    let mut heap: BinaryHeap<State> = BinaryHeap::new();
    let mut nodes: HashMap<i32, LightNode> = HashMap::new();

    let start = Node::from_select(&trans, "WHERE id = $1", &[&starting_node])
        ?.into_iter().nth(0)
        .ok_or("Starting node does not exist.")?;

    nodes.insert(start.id, LightNode { dist: 0.0, parent: None});
    heap.push(State { cost: OrderedFloat(0.0), id: start.id });

    let dest = Node::from_select(&trans, "WHERE id = $1 AND graph_part = $2",
                                 &[&goal_node, &start.graph_part])
        ?.into_iter().nth(0)
        .ok_or(
            "Finishing node does not exist, or is not in the same graph part as the starting node."
        )?;

    let mut considered = 0;
    let mut updated = 0;

    'outer: while let Some(State { cost, id }) = heap.pop() {
        assert!(id != dest.id);
        let dist = nodes.get(&id).unwrap().dist;
        if *cost > dist { continue; }
        assert!(*cost == dist);

        trace!("considering node {} with dist {} ({}c/{}u)", id, dist, considered, updated);

        let links = Link::from_select(&trans, "WHERE p1 = $1 OR p2 = $1", &[&id])?;
        for link in links {
            let tent_dist = link.distance + dist;
            let other_end = if link.p1 == id { link.p2 } else { link.p1 };
            if {
                if let Some(other) = nodes.get_mut(&other_end) {
                    if other.dist > tent_dist {
                        other.dist = tent_dist;
                        other.parent = Some((id, link.way.clone()));
                        heap.push(State { cost: OrderedFloat(tent_dist), id: other_end });
                        updated += 1;
                    }
                    false
                }
                else { true }
            } {
                nodes.insert(other_end, LightNode {
                    dist: tent_dist,
                    parent: Some((id, link.way))
                });
                if other_end == dest.id {
                    break 'outer;
                }
                updated += 1;
                heap.push(State { cost: OrderedFloat(tent_dist), id: other_end });
            }
        }
        considered += 1;
        if (considered % 1000) == 0 {
            debug!("navigate: considered {} nodes ({} updated)", considered, updated);
        }
    }
    let mut path_nodes = vec![dest.id];
    let mut path = vec![];
    let mut cur_node = nodes.get(&dest.id).unwrap();
    let mut crossings = HashSet::new();
    loop {
        let node = Node::from_select(conn, "WHERE id = $1", &[&path_nodes.last().unwrap()])?;
        if let Some(id) = node[0].parent_crossing {
            trace!("navigate: found intersecting crossing #{}", id);
            crossings.insert(id);
        }
        if let Some((ref parent_id, ref geom)) = cur_node.parent {
            let geom: LineString = conn.query(
                "SELECT
                 CASE WHEN ST_Intersects(ST_EndPoint($1), location)
                      THEN $1
                      ELSE ST_Reverse($1)
                 END FROM nodes WHERE id = $2",
                &[&geom, &path_nodes.last().unwrap()])
                ?.into_iter().nth(0).unwrap().get(0);
            path_nodes.push(*parent_id);
            path.insert(0, geom.clone());
            cur_node = nodes.get(parent_id).unwrap();
        }
        else {
            break;
        }
    }
    let crossings = crossings.into_iter().collect::<Vec<_>>();
    let path: LineString = conn.query("SELECT ST_MakeLine(CAST($1 AS geometry[]))", &[&path])
        ?.into_iter().nth(0).unwrap().get(0);
    debug!("navigate: finding intersecting crossings...");
    let mut crossing_locations = vec![];
    for cx in Crossing::from_select(conn, "WHERE id = ANY($1)", &[&crossings])? {
        for row in &trans.query("SELECT ST_Line_Locate_Point($1, ST_Centroid($2))",
                                &[&path, &cx.area])? {
            let location: f64 = row.get(0);
            crossing_locations.push(location);
        }
    }
    debug!("navigate: completed");
    Ok(StationPath {
        s1: from.to_string(), s2: to.to_string(), way: path,
        nodes: path_nodes, crossings, crossing_locations,
        id: -1
    })
}
