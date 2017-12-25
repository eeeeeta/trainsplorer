use osmpbfreader::{OsmPbfReader};
use osmpbfreader::reader::ParIter;
use osmpbfreader::objects::{OsmObj};
use super::errors::*;
use indicatif::{ProgressStyle, ProgressBar};
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

pub struct ImportContext<'a, R: 'a> {
    objs: Option<u64>,
    pool: &'a DbPool,
    n_threads: usize,
    reader: &'a mut OsmPbfReader<R>
}
impl<'a, R> ImportContext<'a, R> where R: Read + Seek {
    pub fn new(rdr: &'a mut OsmPbfReader<R>, pool: &'a DbPool, n_threads: usize) -> Self {
        ImportContext {
            objs: None,
            pool, n_threads,
            reader: rdr
        }
    }
    fn par_iter<'b>(&'b mut self) -> Result<ParIter<'b, R>> {
        self.reader.rewind()?;
        Ok(self.reader.par_iter())
    }
    fn make_bar(&self) -> ProgressBar {
        let bar = ProgressBar::new_spinner();
        if let Some(o) = self.objs {
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
    fn make_custom_bar(&self, len: u64) -> ProgressBar {
        let ret = ProgressBar::new_spinner();
        ret.set_length(len);
        ret.set_style(ProgressStyle::default_bar()
                      .template("[{elapsed_precise}] {bar:40.red/yellow} {pos:>7}/{len:7} {msg}")
                      .progress_chars("##-"));
        ret
    }
    fn get_pool(&self) -> &::r2d2::Pool<::r2d2_postgres::PostgresConnectionManager> {
        self.pool
    }
    fn get_conn(&self) -> ::r2d2::PooledConnection<::r2d2_postgres::PostgresConnectionManager> {
        self.pool.get().unwrap()
    }
    fn n_threads(&self) -> usize {
        self.n_threads
    }
    fn update_objs(&mut self, objs: u64) {
        self.objs = Some(objs);
    }
    fn count(&self, query: &str) -> Result<i64> {
        Ok(util::count(&*self.get_conn(), query, &[])?)
    }
}
pub fn geo_pt_to_postgis(pt: Point<f64>) -> PgPoint {
    PgPoint::new(pt.0.x, pt.0.y, Some(4326))
}
pub fn geo_ls_to_postgis(ls: LineString<f64>) -> PgLineString {
    PgLineString {
        points: ls.0.into_iter().map(geo_pt_to_postgis).collect(),
        srid: Some(4326)
    }
}
pub fn count<R: Read + Seek>(ctx: &mut ImportContext<R>) -> Result<()> {
    let bar = ctx.make_bar();
    bar.set_message("Beginning object count: iterating");
    let mut objs: u64 = 0;
    for _ in ctx.par_iter()? {
        objs += 1;
        bar.set_message(&format!("{} objects counted so far", objs));
    }
    ctx.update_objs(objs);
    bar.finish();
    debug!("{} objects in map file", objs);
    Ok(())
}
pub fn crossings<R: Read + Seek>(ctx: &mut ImportContext<R>) -> Result<()> {
    use geo::algorithm::boundingbox::BoundingBox;

    if ctx.count("FROM crossings")? != 0 { return Ok(()) };
    info!("Phase 1.4: making crossings");
    let todo = ctx.count("FROM nodes WHERE osm_was_crossing = true")?;
    let bar = ctx.make_custom_bar(todo as _);
    bar.set_message("Processing crossing nodes");

    let conn = ctx.get_conn();
    let trans = conn.transaction()?;
    let mut done = Vec::new();
    let mut skipped = 0;

    for nd in Node::from_select(&trans, "WHERE osm_was_crossing = true", &[])? {
        bar.inc(1);
        if done.contains(&nd.id) {
            skipped += 1;
            continue;
        }
        bar.set_message(&format!("Processing node #{} (done = {}, skipped = {})", nd.id, done.len(), skipped));
        let mut nodes = vec![nd.id];
        let mut mp = MultiPoint(vec![Point::from_postgis(&nd.location)]);
        done.push(nd.id);
        for other_nd in Node::from_select(&trans, "WHERE osm_was_crossing = true
                                                   AND ST_Distance(location::geography, $1::geography) < 35",
                                          &[&nd.location])? {
            if !done.contains(&other_nd.id) {
                done.push(other_nd.id);
                nodes.push(other_nd.id);
                mp.0.push(Point::from_postgis(&nd.location));
            }
        }
        let bbox = mp.bbox().ok_or("couldn't find bounding box")?;
        let ls = geo_ls_to_postgis(LineString(vec![
            Point::new(bbox.xmin, bbox.ymin),
            Point::new(bbox.xmin, bbox.ymax),
            Point::new(bbox.xmax, bbox.ymax),
            Point::new(bbox.xmax, bbox.ymin),
        ]));
        let poly = PgPolygon {
            rings: vec![ls],
            srid: Some(4326)
        };
        let cx = Crossing::insert(&trans, None, poly)?;
        for nd in nodes {
            trans.execute("UPDATE nodes SET parent_crossing = $1 WHERE id = $2", &[&cx, &nd])?;
        }
    }
    trans.commit()?;
    Ok(())
}
pub fn nodes<R: Read + Seek>(ctx: &mut ImportContext<R>) -> Result<()> {
    if ctx.count("FROM nodes")? != 0 { return Ok(()) };
    info!("Phase 1.1: making nodes");
    let conn = ctx.get_conn();
    let trans = conn.transaction()?;
    let bar = ctx.make_bar();
    bar.set_message("Beginning node import");
    let mut objs = 0;
    for obj in ctx.par_iter()? {
        bar.inc(1);
        if let OsmObj::Node(nd) = obj? {
            bar.set_message(&format!("Processing node #{}", nd.id.0));
            let lat = nd.decimicro_lat as f64 / 10_000_000.0;
            let lon = nd.decimicro_lon as f64 / 10_000_000.0;
            let pt = PgPoint::new(lon, lat, Some(4326));
            let osm_was_crossing = nd.tags.contains("railway", "level_crossing");
            let node = Node {
                id: -1,
                location: pt,
                graph_part: 0,
                parent_crossing: None,
                orig_osm_id: Some(nd.id.0),
                osm_was_crossing
            };
            node.insert_self(&trans)?;
        }
        objs += 1;
    }
    ctx.update_objs(objs);
    trans.commit()?;
    Ok(())
}
pub fn links<R: Read + Seek>(ctx: &mut ImportContext<R>) -> Result<()> {
    use geo::algorithm::haversine_length::HaversineLength;
    use self::chase_lev::Steal;

    if ctx.count("FROM links")? != 0 { return Ok(()) };
    info!("Phase 1.2: making links");

    let bar = ctx.make_bar();
    bar.set_message("Beginning link import");
    let (worker, stealer) = chase_lev::deque();

    let mut objs = 0;
    let mut ways = 0;
    for obj in ctx.par_iter()? {
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
    ctx.update_objs(objs);
    let bar = ctx.make_custom_bar(ways);
    let pool = ctx.get_pool();
    bar.set_message(&format!("Processing ways using {} threads...", ctx.n_threads()));
    ::crossbeam::scope(|scope| {
        for n in 0..ctx.n_threads() {
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
    Ok(())
}
pub fn stations<R: Read + Seek>(ctx: &mut ImportContext<R>) -> Result<()> {
    use geo::algorithm::haversine_destination::HaversineDestination;
    use geo::algorithm::haversine_length::HaversineLength;
    use geo::algorithm::centroid::Centroid;

    if ctx.count("FROM stations")? != 0 { return Ok(()) };
    info!("Phase 1.3: making stations");

    let conn = ctx.get_conn();
    let trans = conn.transaction()?;
    let bar = ctx.make_bar();
    bar.set_message("Beginning station import");
    let mut objs = 0;
    let mut polys = HashMap::new();
    'outer: for obj in ctx.par_iter()? {
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
    ctx.update_objs(objs);
    bar.finish();
    let bar = ctx.make_custom_bar(polys.len() as _);
    bar.set_message("Making stations");
    for (nr_ref, poly) in polys {
        bar.set_message(&format!("Processing station {}", nr_ref));
        let centroid = poly.centroid().ok_or("Station has no centroid")?;
        let pgpoly = PgPolygon {
            rings: vec![geo_ls_to_postgis(poly.exterior.clone())],
            srid: Some(4326)
        };
        let nd = Node::insert(&trans, geo_pt_to_postgis(centroid))?;
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
    Ok(())
}
pub fn separate_nodes<R: Read + Seek>(ctx: &mut ImportContext<R>) -> Result<()> {
    let todo = ctx.count("FROM nodes WHERE graph_part = 0")?;
    if todo == 0 { return Ok(()) };
    info!("Phase 1.5: separating nodes");

    let conn = ctx.get_conn();
    let trans = conn.transaction()?;
    let bar = ctx.make_custom_bar(todo as _);
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
