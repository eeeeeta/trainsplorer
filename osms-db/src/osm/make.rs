use super::types::*;
use db::{GenericConnection, DbPool, DbType};
use postgis::ewkb::{Point, Polygon};
use postgres::transaction::Transaction;
use geo;
use util::*;
use errors::*;
use std::collections::HashMap;
use std::thread;
use std::sync::atomic::{Ordering, AtomicUsize};
use std::sync::Arc;
use std::time::Instant;
use std::sync::mpsc::channel;
use crossbeam::sync::chase_lev;
use chashmap::CHashMap;
use ordered_float::OrderedFloat;

fn split_links<A, B, T>(conn: &T, links_points: A, on_make: B, todo: Option<usize>) -> Result<()>
    where A: IntoIterator<Item=((i64, i64), Vec<geo::Point<f64>>)>,
          B: Fn(&Transaction, geo::Point<f64>, i64) -> Result<()>,
          T: GenericConnection
{

    use geo::algorithm::haversine_length::HaversineLength;
    use geo::algorithm::split::Split;

    let trans = conn.transaction()?;
    let mut done = 0;

    for ((p1, p2), mut points) in links_points.into_iter() {
        let instant = Instant::now();

        trace!("split_links: breaking link {} <-> {} with {} point(s)", p1, p2, points.len());
        let link = Link::from_select(&trans, "WHERE p1 = $1 AND p2 = $2", &[&p1, &p2])?.into_iter()
            .nth(0).unwrap();
        let geoway = geo::LineString::from_postgis(&link.way);
        if geoway.0.len() == 0 {
            warn!("split_links: link {} <-> {} has no points!", p1, p2);
            continue;
        }
        for p in points.iter_mut() {
            if p == geoway.0.first().unwrap() {
                warn!("split_links: point {:?} (#{}) is the start point", p, p1);
                on_make(&trans, *p, p1)?;
                continue;
            }
            else if p == geoway.0.last().unwrap() {
                warn!("split_links: point {:?} (#{}) is the end point", p, p2);
                on_make(&trans, *p, p2)?;
                continue;
            }
            let spl = geoway.split(&p, 0.00000001);
            if spl.len() != 2 {
                warn!("split_links: point {:?} splits into {} piece(s), not 2!", p, spl.len());
                debug!("split_links: way={:?}", geoway);
                debug!("split_links: split={:?}", spl);
                p.0.x = 0.0;
                p.0.y = 0.0;
                continue;
            }
        }
        points.retain(|p| {
            p.0.x != 0.0 && p.0.y != 0.0 && p != geoway.0.first().unwrap() && p != geoway.0.last().unwrap()
        });
        if points.len() == 0 {
            warn!("split_links: no points left");
            continue;
        }
        // make sure the points are in order of their incidence along the line
        points.sort_by(|p1, p2| {
            let p1_split = geoway.split(&p1, 0.00000001);
            let p2_split = geoway.split(&p2, 0.00000001);
            assert!(p1_split.len() == 2 && p2_split.len() == 2);
            let p1_segm = p1_split.into_iter().next().unwrap();
            let p2_segm = p2_split.into_iter().next().unwrap();
            p1_segm.haversine_length().partial_cmp(&p2_segm.haversine_length())
                .unwrap()
        });

        let mut prev = (p1, geoway.0[0]);
        for (i, point) in points.into_iter().enumerate() {
            let n2 = Node::insert_processed(&trans, geo_pt_to_postgis(point.clone()), true)?;
            on_make(&trans, point, n2)?;
            let way = geoway.split(&prev.1, 0.00000001)
                .into_iter().nth(1).unwrap();
            let way = way.split(&point, 0.00000001)
                .into_iter().nth(0).unwrap();
            trace!("split_links: making link {} <-> {}", prev.0, n2);
            if i == 0 {
                let dist = way.haversine_length();
                trans.execute(
                    "UPDATE links SET p2 = $1, way = $2, distance = $3 WHERE p1 = $4 AND p2 = $5",
                    &[&n2, &geo_ls_to_postgis(way), &(dist as f32), &p1, &p2]
                )?;
            }
            else {
                let new = Link {
                    p1: prev.0,
                    p2: n2,
                    distance: way.haversine_length() as _,
                    way: geo_ls_to_postgis(way)
                };
                new.insert(&trans)?;
            }
            prev = (n2, point);
        }
        trace!("split_links: making last link {} <-> {}", prev.0, p2);
        let way = geoway.split(&prev.1, 0.00000001)
            .into_iter().nth(1).unwrap();
        let new = Link {
            p1: prev.0,
            p2: p2,
            distance: way.haversine_length() as _,
            way: geo_ls_to_postgis(way)
        };
        new.insert(&trans)?;

        if let Some(todo) = todo {
            let now = Instant::now();
            let dur = now.duration_since(instant);
            let dur = dur.as_secs() as f64 + dur.subsec_nanos() as f64 * 1e-9;
            done += 1;
            debug!("split_links: {} of {} complete ({:.01}%) - time: {:.04}s", done, todo, (done as f64 / todo as f64) * 100.0, dur);
        }
    }
    trans.commit()?;
    Ok(())
}

pub fn make_crossings(pool: &DbPool, n_threads: usize) -> Result<()> {
    debug!("make_crossings: running...");
    let conn = pool.get().unwrap();
    let todo = count(&*conn, "FROM planet_osm_point WHERE railway = 'level_crossing'", &[])?;
    debug!("make_crossings: {} crossings to consider", todo);

    let (worker, stealer) = chase_lev::deque();
    let links_points = Arc::new(CHashMap::new());
    let points_crossings = Arc::new(CHashMap::new());
    let done = Arc::new(AtomicUsize::new(0));

    for row in &conn.query("SELECT osm_id, way, ST_Buffer(way::geography, 20), name FROM planet_osm_point WHERE railway = 'level_crossing'", &[])? {
        let (osm_id, way, area, name): (i64, Point, Polygon, Option<String>)
            = (row.get(0), row.get(1), row.get(2), row.get(3));
        worker.push((osm_id, way, area, name));
    }

    let mut threads = vec![];
    debug!("make_crossings: starting phase 1: figuring out crossing points");
    for n in 0..n_threads {
        debug!("make_crossings: spawning thread {}", n);
        let p = pool.clone();
        let d = done.clone();
        let s = stealer.clone();
        let links_points = links_points.clone();
        let points_crossings = points_crossings.clone();
        threads.push(thread::spawn(move|| {
            let db = p.get().unwrap();
            loop {
                match s.steal() {
                    chase_lev::Steal::Empty => {
                        debug!("make_crossings: thread {} done", n);
                        break;
                    },
                    chase_lev::Steal::Data((osm_id, way, area, name)) => {
                        let instant = Instant::now();
                        let geopt = geo::Point::from_postgis(&way);
                        let key = (OrderedFloat(geopt.0.x), OrderedFloat(geopt.0.y));
                        if let Some(x) = points_crossings.insert(key, 0) {
                            points_crossings.insert(key, x);
                        }
                        else {
                            let mut points = vec![(osm_id, way)];
                            for row in &db.query("SELECT osm_id, way FROM planet_osm_point WHERE railway = 'level_crossing' AND ST_Intersects(way, $1)", &[&area]).unwrap() {
                                let (osm_id, way): (i64, Point) = (row.get(0), row.get(1));
                                let geopt = geo::Point::from_postgis(&way);
                                let key = (OrderedFloat(geopt.0.x), OrderedFloat(geopt.0.y));
                                if let Some(x) = points_crossings.insert(key, 0) {
                                    points_crossings.insert(key, x);
                                }
                                else {
                                    points.push((osm_id, way));
                                }
                            }
                            let crossing = Crossing::insert(&*db, name, area)
                                .unwrap();
                            for (osm_id, way) in points {
                                let links = Link::from_select(&*db, "WHERE ST_Intersects(way, $1)", &[&way])
                                    .unwrap();
                                if links.len() == 0 {
                                    warn!("*** Crossing with OSM ID #{} doesn't connect to anything!", osm_id);
                                }
                                for link in links {
                                    let geopt = geo::Point::from_postgis(&way);
                                    links_points.upsert((link.p1, link.p2), || vec![geopt], |vec| vec.push(geopt));
                                    let key = (OrderedFloat(geopt.0.x), OrderedFloat(geopt.0.y));
                                    points_crossings.insert(key, crossing);
                                }
                            }
                        }
                        let now = Instant::now();
                        let dur = now.duration_since(instant);
                        let dur = dur.as_secs() as f64 + dur.subsec_nanos() as f64 * 1e-9;
                        let done = d.fetch_add(1, Ordering::SeqCst) + 1;
                        debug!("make_crossings: {} of {} crossings almost complete ({:.01}%) - time: {:.04}s", done, todo, (done as f64 / todo as f64) * 100.0, dur);
                    },
                    _ => {}
                }
            }
        }));
    }
    for thr in threads {
        thr.join().unwrap();
    }
    debug!("make_crossings: starting phase 2: splitting up links");
    let on_make = |trans: &Transaction, point: geo::Point<f64>, id: i64| -> Result<()> {
        let key = (OrderedFloat(point.0.x), OrderedFloat(point.0.y));
        let crossing = points_crossings.get(&key).unwrap();
        trans.execute("UPDATE nodes SET parent_crossing = $1 WHERE id = $2", &[&*crossing, &id])?;
        Ok(())
    };
    let todo = links_points.len();
    split_links(&*conn, Arc::try_unwrap(links_points).unwrap(), on_make, Some(todo))?;
    Ok(())
}
pub fn make_stations(pool: &DbPool, n_threads: usize) -> Result<()> {
    use geo::algorithm::closest_point::ClosestPoint;

    let conn = pool.get().unwrap();
    let mut areas: HashMap<String, (Polygon, Point)> = HashMap::new();
    for row in &conn.query(
        "SELECT ref, way, ST_Centroid(way)
         FROM planet_osm_polygon
         WHERE railway = 'station' AND ref IS NOT NULL", &[])? {

        areas.insert(row.get(0), (row.get(1), row.get(2)));
    }
    for row in &conn.query(
        "SELECT ref, ST_Buffer(way::geography, 50)::geometry, way
         FROM planet_osm_point
         WHERE railway = 'station' AND ref IS NOT NULL", &[])? {

        areas.insert(row.get(0), (row.get(1), row.get(2)));
    }
    let done = Arc::new(AtomicUsize::new(0));
    let (worker, stealer) = chase_lev::deque();
    let mut threads = vec![];
    debug!("make_stations: {} stations to process", areas.len());
    let todo = areas.len();
    for (nr_ref, (poly, point)) in areas {
        worker.push((nr_ref, (poly, point)));
    }
    let (tx, rx) = channel::<Option<(String, i64, Polygon)>>();
    let p = pool.clone();
    debug!("make_stations: starting phase 1: figuring out station points");
    let endthr = thread::spawn(move || {
        let db = p.get().unwrap();
        let trans = db.transaction().unwrap();
        debug!("make_stations: spawning inserter thread");
        while let Some((nr_ref, node, poly)) = rx.recv().unwrap() {
            Station::insert(&trans, &nr_ref, node, poly).unwrap();
        }
        trans.commit().unwrap();
        debug!("make_stations: inserter thread done");
    });
    let links_points = Arc::new(CHashMap::new());
    let point_stations = Arc::new(CHashMap::new());
    for n in 0..n_threads {
        debug!("make_stations: spawning thread {}", n);
        let p = pool.clone();
        let d = done.clone();
        let s = stealer.clone();
        let lp = links_points.clone();
        let ps = point_stations.clone();
        let tx = tx.clone();
        threads.push(thread::spawn(move || {
            let db = p.get().unwrap();
            loop {
                match s.steal() {
                    chase_lev::Steal::Empty => {
                        debug!("make_stations: thread {} done", n);
                        break;
                    },
                    chase_lev::Steal::Data((nr_ref, (poly, point))) => {
                        debug!("make_stations: processing station {}", nr_ref);
                        let instant = Instant::now();

                        let pt = geo::Point::from_postgis(&point);
                        let station = Node::insert_processed(&*db, point, true)
                            .unwrap();
                        let links = Link::from_select(&*db, "WHERE ST_Intersects(way, $1)", &[&poly])
                            .unwrap();
                        if links.len() == 0 {
                            warn!("*** Station {} doesn't connect to anything!", nr_ref);
                        }
                        for link in links {
                            let geoway = geo::LineString::from_postgis(&link.way);
                            let geocp = geoway.closest_point(&pt)
                                .expect("closest_point() returned None");
                            lp.upsert((link.p1, link.p2), || vec![geocp], |vec| vec.push(geocp));
                            let key = (OrderedFloat(geocp.0.x), OrderedFloat(geocp.0.y));
                            ps.upsert(key, || vec![station], |vec| vec.push(station));
                        }
                        tx.send(Some((nr_ref, station, poly))).unwrap();

                        let now = Instant::now();
                        let dur = now.duration_since(instant);
                        let dur = dur.as_secs() as f64 + dur.subsec_nanos() as f64 * 1e-9;
                        let done = d.fetch_add(1, Ordering::SeqCst) + 1;
                        debug!("make_stations: {} of {} stations almost complete ({:.01}%) - time: {:.04}s", done, todo, (done as f64 / todo as f64) * 100.0, dur);
                    },
                    _ => {}
                }
            }
        }));
    }
    for thr in threads {
        thr.join().unwrap();
    }
    tx.send(None).unwrap();
    endthr.join().unwrap();
    debug!("make_stations: starting phase 2: splitting up links");
    let on_make = |trans: &Transaction, point: geo::Point<f64>, id: i64| -> Result<()> {
        let key = (OrderedFloat(point.0.x), OrderedFloat(point.0.y));
        let stations = point_stations.get(&key).unwrap();
        for station in stations.iter() {
            let connection = geo::LineString(vec![]);
            let link = Link {
                p1: *station,
                p2: id,
                distance: 0.0,
                way: geo_ls_to_postgis(connection)
            };
            link.insert(trans)?;
        }
        Ok(())
    };
    let todo = links_points.len();
    split_links(&*conn, Arc::try_unwrap(links_points).unwrap(), on_make, Some(todo))?;
    Ok(())
}

pub fn make_nodes<T: GenericConnection>(conn: &T) -> Result<()> {
    debug!("make_nodes: making nodes from OSM data...");
    let trans = conn.transaction()?;
    let mut compl = 0;
    for row in &trans.query("SELECT ST_StartPoint(way), ST_EndPoint(way)
                            FROM planet_osm_line WHERE railway = 'rail'", &[])? {
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
pub fn make_links(pool: &DbPool, n_threads: usize) -> Result<()> {
    debug!("make_links: making links from OSM data...");
    let conn = pool.get().unwrap();
    let links_made = count(&*conn, "FROM links", &[])?;
    let nodes_processed = count(&*conn, "FROM nodes WHERE processed = true", &[])?;
    if links_made == 0 && nodes_processed > 0 {
        debug!("make_links: cleaning up after interrupted previous run...");
        conn.execute("DELETE FROM nodes", &[])?;
        debug!("make_links: done cleaning up");
    }
    let todo = count(&*conn, "FROM nodes WHERE processed = false", &[])?;
    debug!("make_links: {} nodes to make links for", todo);
    let done = Arc::new(AtomicUsize::new(0));
    let mut threads = vec![];
    let p = pool.clone();
    let (tx, rx) = channel::<Option<Link>>();
    debug!("make_links: spawning inserter thread");
    let endthr = thread::spawn(move || {
        let db = p.get().unwrap();
        let trans = db.transaction().unwrap();
        while let Some(link) = rx.recv().unwrap() {
            link.insert(&trans).unwrap();
        }
        trans.commit().unwrap();
        debug!("make_links: inserter thread done");
    });
    for n in 0..n_threads {
        debug!("make_links: spawning thread {}", n);
        let p = pool.clone();
        let d = done.clone();
        let tx = tx.clone();
        threads.push(thread::spawn(move || {
            let db = p.get().unwrap();
            loop {
                let trans = db.transaction().unwrap();
                let nodes = Node::from_select(&trans, "WHERE processed = false LIMIT 1
                                                       FOR UPDATE SKIP LOCKED", &[])
                    .unwrap();
                if nodes.len() == 0 {
                    debug!("make_links: thread {} done", n);
                    break;
                }
                for node in nodes {
                    let instant = Instant::now();
                    for row in &trans.query(
                        "SELECT way, CAST(ST_Length(way::geography, false) AS REAL), id
                         FROM planet_osm_line
                         INNER JOIN nodes ON ST_EndPoint(planet_osm_line.way) = nodes.location
                         WHERE railway = 'rail' AND ST_Intersects(ST_StartPoint(way), $1)",
                        &[&node.location]).unwrap() {
                        let link = Link { p1: node.id, p2: row.get(2), way: row.get(0), distance: row.get(1) };
                        tx.send(Some(link)).unwrap();
                    }
                    trans.execute("UPDATE nodes SET processed = true WHERE id = $1", &[&node.id])
                        .unwrap();
                    let now = Instant::now();
                    let dur = now.duration_since(instant);
                    let dur = dur.as_secs() as f64 + dur.subsec_nanos() as f64 * 1e-9;
                    let done = d.fetch_add(1, Ordering::SeqCst) + 1;
                    debug!("make_links: {} of {} nodes complete ({:.01}%) - time: {:.04}s", done, todo, (done as f64 / todo as f64) * 100.0, dur);
                }
                trans.commit().unwrap();
            }
        }));
    }
    for thr in threads {
        thr.join().unwrap();
    }
    tx.send(None).unwrap();
    endthr.join().unwrap();
    debug!("make_links: complete!");
    Ok(())
}
