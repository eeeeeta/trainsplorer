use super::types::*;
use db::{GenericConnection, DbType};
use errors::*;
use std::collections::HashSet;
use util::*;
use std::time::Instant;

pub fn separate_nodes<T: GenericConnection>(conn: &T) -> Result<()> {
    debug!("separate_nodes: running...");
    let trans = conn.transaction()?;
    let todo = count(&trans, "FROM nodes WHERE graph_part = 0", &[])?;
    let mut done = 0;
    debug!("separate_nodes: {} nodes to separate", todo);
    let mut cur_graph_part = 1;
    loop {
        let instant = Instant::now();

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
        cur_graph_part += 1;
        done += nodes_touched;
        let now = Instant::now();
        let dur = now.duration_since(instant);
        let dur = dur.as_secs() as f64 + dur.subsec_nanos() as f64 * 1e-9;
        debug!("separate_nodes: done part {} ({} nodes) - {} of {} nodes touched ({:.01}%) - time: {:.04}s",
               cur_graph_part, nodes_touched, done, todo, (done as f64 / todo as f64) * 100.0, dur);
    }
    trans.commit()?;
    debug!("separate_nodes: separated graph into {} parts", cur_graph_part);
    Ok(())
}
