use osmpbfreader::{OsmPbfReader};
use osmpbfreader::objects::{OsmObj};
use super::errors::*;
use indicatif::{ProgressStyle, ProgressBar};
use postgres::GenericConnection;
use std::io::{Read, Seek};
use geo::*;
use std::collections::HashSet;
use postgis::ewkb::Point as PgPoint;
use postgis::ewkb::LineString as PgLineString;
use postgis::ewkb::Polygon as PgPolygon;
use osms_db::db::*;
use osms_db::util;
use osms_db::osm::types::*;
use crossbeam::sync::chase_lev;
use std::collections::HashMap;

pub fn geo_pt_to_postgis(pt: Point<f64>) -> PgPoint {
    PgPoint::new(pt.0.x, pt.0.y, Some(4326))
}
pub fn geo_ls_to_postgis(ls: LineString<f64>) -> PgLineString {
    PgLineString {
        points: ls.0.into_iter().map(geo_pt_to_postgis).collect(),
        srid: Some(4326)
    }
}
pub fn make_bar(objs: Option<u64>) -> ProgressBar {
    let bar = ProgressBar::new_spinner();
    if let Some(o) = objs {
        bar.set_length(o);
        bar.set_style(ProgressStyle::default_bar()
                      .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
                      .progress_chars("##-"));
    }
    else {
        bar.set_style(ProgressStyle::default_bar()
                      .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/??? {msg}")
                      .progress_chars("##-"));

    }
    bar
}
pub fn count<R: Read + Seek>(rdr: &mut OsmPbfReader<R>) -> Result<u64> {
    let bar = ProgressBar::new_spinner();
    bar.set_message("Beginning object count: rewinding file");
    rdr.rewind()?;
    bar.set_message("Beginning object count: iterating");
    let mut objs: u64 = 0;
    for _ in rdr.par_iter() {
        objs += 1;
        bar.set_message(&format!("{} objects counted so far", objs));
    }
    bar.finish();
    debug!("{} objects in map file", objs);
    Ok(objs)
}
pub fn nodes<T: GenericConnection, R: Read + Seek>(conn: &T, rdr: &mut OsmPbfReader<R>, objs: Option<u64>) -> Result<u64> {
    let trans = conn.transaction()?;
    let bar = make_bar(objs);
    bar.set_message("Beginning node import: rewinding file");
    rdr.rewind()?;
    bar.set_message("Beginning node import: iterating");
    let mut objs = 0;
    for obj in rdr.par_iter() {
        bar.inc(1);
        if let OsmObj::Node(nd) = obj? {
            bar.set_message(&format!("Processing node #{}", nd.id.0));
            let lat = nd.decimicro_lat as f64 / 10_000_000.0;
            let lon = nd.decimicro_lon as f64 / 10_000_000.0;
            let pt = PgPoint::new(lon, lat, Some(4326));
            Node::insert_from_osm(&trans, pt, nd.id.0)?;
        }
        objs += 1;
    }
    trans.commit()?;
    Ok(objs)
}
pub fn links<R: Read + Seek>(pool: &DbPool, n_threads: usize, rdr: &mut OsmPbfReader<R>, objs: Option<u64>) -> Result<u64> {
    use geo::algorithm::haversine_length::HaversineLength;
    use self::chase_lev::Steal;

    let bar = make_bar(objs);
    bar.set_message("Beginning link import: rewinding file");
    rdr.rewind()?;
    bar.set_message("Beginning link import: iterating");
    let (worker, stealer) = chase_lev::deque();

    let mut objs = 0;
    let mut ways = 0;
    for obj in rdr.par_iter() {
        bar.inc(1);
        if let OsmObj::Way(way) = obj? {
            if way.tags.contains("railway", "rail") {
                bar.set_message(&format!("Enqueueing way #{}", way.id.0));
                worker.push(way);
                ways += 1;
            }
        }
        objs += 1;
    }
    bar.finish();
    let bar = make_bar(Some(ways));
    bar.set_message(&format!("Processing ways using {} threads...", n_threads));
    ::crossbeam::scope(|scope| {
        for n in 0..n_threads {
            debug!("links: spawning thread {}", n);
            scope.spawn(|| {
                'outer: loop {
                    let db = pool.get().unwrap();
                    match stealer.steal() {
                        Steal::Empty => {
                            debug!("links: thread done");
                            bar.finish();
                            break;
                        },
                        Steal::Data(way) => {
                            let trans = db.transaction().unwrap();
                            for slice in way.nodes.windows(2) {
                                let p1 = Node::from_select(&trans, "WHERE orig_osm_id = $1", &[&slice[0].0])
                                    .unwrap().into_iter()
                                    .nth(0);
                                let p1 = match p1 {
                                    Some(n) => n,
                                    None => {
                                        debug!("links: way #{} contained invalid point #{}",
                                               way.id.0, slice[0].0);
                                        continue 'outer;
                                    }
                                };
                                let p2 = Node::from_select(&trans, "WHERE orig_osm_id = $1", &[&slice[1].0])
                                    .unwrap().into_iter()
                                    .nth(0);
                                let p2 = match p2 {
                                    Some(n) => n,
                                    None => {
                                        debug!("links: way #{} contained invalid point #{}",
                                               way.id.0, slice[1].0);
                                        continue 'outer;
                                    }
                                };
                                let geo_p1 = Point::from_postgis(&p1.location);
                                let geo_p2 = Point::from_postgis(&p2.location);
                                let ls = LineString(vec![geo_p1, geo_p2]);
                                let dist = ls.haversine_length();

                                let link = Link {
                                    p1: p1.id,
                                    p2: p2.id,
                                    way: geo_ls_to_postgis(ls),
                                    distance: dist as _
                                };
                                link.insert(&trans).unwrap();
                            }
                            trans.commit().unwrap();
                            bar.inc(1);
                        },
                        _ => {}
                    }
                }
            });
        }
    });
    Ok(objs)
}
pub fn stations<T: GenericConnection, R: Read + Seek>(conn: &T, rdr: &mut OsmPbfReader<R>, objs: Option<u64>) -> Result<u64> {
    use geo::algorithm::haversine_destination::HaversineDestination;
    use geo::algorithm::haversine_length::HaversineLength;
    use geo::algorithm::centroid::Centroid;

    let trans = conn.transaction()?;
    let bar = make_bar(objs);
    bar.set_message("Beginning station import: rewinding file");
    rdr.rewind()?;
    bar.set_message("Beginning station import: iterating");
    let mut objs = 0;
    let mut polys = HashMap::new();
    'outer: for obj in rdr.par_iter() {
        bar.inc(1);
        let obj = obj?;
        if obj.tags().contains("railway", "station") && obj.tags().get("ref").is_some() {
            match obj {
                OsmObj::Way(way) => {
                    bar.set_message(&format!("Processing way #{} ({} polys thus far)",
                                             way.id.0, polys.len()));
                    if way.is_closed() {
                        let mut nodes = vec![];
                        for nd in way.nodes.iter() {
                            let pt = Node::from_select(&trans, "WHERE orig_osm_id = $1", &[&nd.0])?.into_iter()
                                .nth(0);
                            let pt = match pt {
                                Some(n) => n,
                                None => {
                                    debug!("stations: way #{} contained invalid point #{}",
                                           way.id.0, nd.0);
                                    continue 'outer;
                                }
                            };
                            nodes.push(Point::from_postgis(&pt.location));
                        }
                        let poly = Polygon { exterior: LineString(nodes), interiors: vec![] };
                        polys.insert(way.tags.get("ref").unwrap().clone(), poly);
                    }
                },
                OsmObj::Node(nd) => {
                    bar.set_message(&format!("Processing node #{} ({} polys thus far)",
                                             nd.id.0, polys.len()));
                    let mut nodes = vec![];
                    let lat = nd.decimicro_lat as f64 / 10_000_000.0;
                    let lon = nd.decimicro_lon as f64 / 10_000_000.0;
                    let pt = Point::new(lon, lat);
                    for bearing in 0..361 {
                        nodes.push(pt.haversine_destination(bearing as _, 50.0));
                    }
                    let poly = Polygon { exterior: LineString(nodes), interiors: vec![] };
                    polys.insert(nd.tags.get("ref").unwrap().clone(), poly);
                },
                _ => {}
            }
        }
        objs += 1;
    }
    bar.finish();
    let bar = make_bar(Some(polys.len() as _));
    bar.set_message("Making stations");
    for (nr_ref, poly) in polys {
        bar.set_message(&format!("Processing station {}", nr_ref));
        let centroid = poly.centroid().ok_or("Station has no centroid")?;
        let pgpoly = PgPolygon {
            rings: vec![geo_ls_to_postgis(poly.exterior.clone())],
            srid: Some(4326)
        };
        let nd = Node::insert_processed(&trans, geo_pt_to_postgis(centroid), true)?;
        Station::insert(&trans, &nr_ref, nd, pgpoly.clone())?;
        for pt in Node::from_select(&trans, "WHERE ST_Intersects(location, $1)", &[&pgpoly])? {
            let geopt = Point::from_postgis(&pt.location);
            let link = LineString(vec![poly.exterior.0[0], geopt]);
            let dist = link.haversine_length();
            let link = Link {
                p1: nd,
                p2: pt.id,
                way: geo_ls_to_postgis(link),
                distance: dist as _
            };
            link.insert(&trans)?;
        }
        bar.inc(1);
    }
    trans.commit()?;
    Ok(objs)
}
pub fn separate_nodes<T: GenericConnection>(conn: &T) -> Result<()> {
    let trans = conn.transaction()?;
    let todo = util::count(&trans, "FROM nodes WHERE graph_part = 0", &[])?;
    let bar = make_bar(Some(todo as _));
    let mut cur_graph_part = 1;
    let mut total = 0;
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
                bar.set_message(&format!("Processing graph part {}: {} nodes so far",
                                         cur_graph_part, part_of_this.len()));
                bar.set_position((total + part_of_this.len()) as _);
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
        total += nodes_touched;
    }
    trans.commit()?;
    debug!("separate_nodes: separated graph into {} parts", cur_graph_part);
    Ok(())
}
