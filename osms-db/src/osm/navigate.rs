use super::types::*;
use db::{GenericConnection, DbType, InsertableDbType};
use errors::*;
use postgis::ewkb::LineString;
use std::cmp::Ordering;
use std::collections::{HashMap, BinaryHeap};
use ordered_float::OrderedFloat;

struct LightNode {
    parent: Option<(i64, LineString)>,
    dist: f32
}
#[derive(Copy, Clone, Eq, PartialEq)]
struct State {
    cost: OrderedFloat<f32>,
    id: i64
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
pub fn navigate_cached<T: GenericConnection>(conn: &T, from: i32, to: i32) -> Result<i32> {
    let paths = StationPath::from_select(conn, "WHERE s1 = $1 AND s2 = $2 FOR UPDATE", &[&from, &to])?;
    if paths.len() > 0 {
        debug!("navigate_cached: returning memoized path from {} to {}", from, to);
        return Ok(paths.into_iter().nth(0).unwrap().id);
    }
    let path = navigate(conn, from, to)?;
    debug!("navigate_cached: memoizing path");
    Ok(path.insert_self(conn)?)
}
pub fn navigate<T: GenericConnection>(conn: &T, from: i32, to: i32) -> Result<StationPath> {
    let start = RailwayLocation::from_select(conn, "WHERE id = $1", &[&from])?.into_iter()
        .nth(0).ok_or(OsmsError::StationNotFound(from.into()))?;
    let starting_node = start.point;

    let goal = RailwayLocation::from_select(conn, "WHERE id = $1", &[&to])?.into_iter()
        .nth(0).ok_or(OsmsError::StationNotFound(to.into()))?;
    let goal_node = goal.point;

    debug!("navigate: navigating from '{}' #{} ({}) to '{}' #{} ({})",
           start.name, start.id, starting_node, goal.name, goal.id, goal_node);

    let mut heap: BinaryHeap<State> = BinaryHeap::new();
    let mut nodes: HashMap<i64, LightNode> = HashMap::new();

    let start = Node::from_select(conn, "WHERE id = $1", &[&starting_node])
        ?.into_iter().nth(0)
        .ok_or(OsmsError::DatabaseInconsistency("station's node doesn't exist"))?;

    nodes.insert(start.id, LightNode { dist: 0.0, parent: None});
    heap.push(State { cost: OrderedFloat(0.0), id: start.id });

    let dest = Node::from_select(conn, "WHERE id = $1 AND graph_part = $2",
                                 &[&goal_node, &start.graph_part])
        ?.into_iter().nth(0)
        .ok_or(OsmsError::IncorrectGraphPart {
            from: from.into(),
            to: to.into()
        })?;

    let mut considered = 0;
    let mut updated = 0;

    'outer: while let Some(State { cost, id }) = heap.pop() {
        assert!(id != dest.id);
        let dist = nodes.get(&id).unwrap().dist;
        if *cost > dist { continue; }
        assert!(*cost == dist);

        trace!("considering node {} with dist {} ({}c/{}u)", id, dist, considered, updated);

        let links = Link::from_select(conn, "WHERE p1 = $1 OR p2 = $1", &[&id])?;
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
    loop {
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
    let path: LineString = conn.query("SELECT ST_MakeLine(CAST($1 AS geometry[]))", &[&path])
        ?.into_iter().nth(0).unwrap().get(0);
    debug!("navigate: completed");
    Ok(StationPath {
        s1: from, s2: to, way: path,
        nodes: path_nodes,
        id: -1
    })
}
