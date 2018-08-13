use osmpbfreader::{OsmPbfReader};
use osmpbfreader::reader::ParIter;
use osmpbfreader::objects::OsmObj;
use indicatif::{ProgressStyle, ProgressBar};
use std::io::{Read, BufRead, Seek, BufReader};
use geo::*;
use std::collections::HashSet;
use postgis::ewkb::Point as PgPoint;
use postgis::ewkb::Polygon as PgPolygon;
use osms_db::db::*;
use osms_db::util;
use osms_db::osm::types::*;
use osms_db::errors::OsmsError;
use osms_db::ntrod::types::*;
use ntrod_types::reference::{CorpusEntry, CorpusData};
use ntrod_types::schedule::{self, ScheduleRecord};
use failure::Error;
use std::collections::HashMap;

type Result<T> = ::std::result::Result<T, Error>;

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
    use geo::algorithm::from_postgis::FromPostgis;
    use geo::algorithm::to_postgis::ToPostgis;

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
        let bbox = mp.bbox().ok_or(format_err!("couldn't find bounding box"))?;
        let poly = util::geo_bbox_to_poly(bbox).to_postgis_wgs84();
        let cx = Crossing::insert(&trans, None, poly)?;
        for nd in nodes {
            trans.execute("UPDATE nodes SET parent_crossing = $1 WHERE id = $2", &[&cx, &nd])?;
        }
    }
    trans.commit()?;
    Ok(())
}
#[derive(Deserialize)]
pub struct NaptanCsv {
    #[serde(rename = "AtcoCode")]
    atcocode: String,
    #[serde(rename = "TiplocCode")]
    tiploccode: String,
    #[serde(rename = "CrsCode")]
    crscode: String,
    #[serde(rename = "StationName")]
    stationname: String,
    #[serde(rename = "Easting")]
    easting: u32,
    #[serde(rename = "Northing")]
    northing: u32
}
pub fn msn_entries<R: BufRead, T: GenericConnection>(conn: &T, file: R) -> Result<()> {
    use atoc_msn::*;
    use atoc_msn::types::*;

    let trans = conn.transaction()?;
    info!("Importing MSN entries...");
    info!("(that's Master Station Names, not Microsoft Network...)");
    let mut done = 0;
    for line in file.lines() {
        let line = line?;
        if let Ok((_, data)) = msn_record(&line) {
            match data {
                MsnRecord::Header(h) => {
                    debug!("msn_entries: file creation timestamp {}", h.timestamp); 
                },
                MsnRecord::Station(s) => {
                    let me = MsnEntry {
                        tiploc: s.tiploc,
                        name: s.name,
                        cate: s.cate_type as _,
                        crs: s.crs    
                    };
                    me.insert_self(&trans)?;
                    done += 1;
                },
                _ => {}
            }
        }
    }
    debug!("msn_entries: imported {} records", done);
    trans.commit()?;
    Ok(())
}
pub fn corpus_entries<R: Read, T: GenericConnection>(conn: &T, file: R) -> Result<()> {
    info!("corpus_entries: Importing CORPUS entries...");
    let data: CorpusData = ::serde_json::from_reader(file)?;
    let mut inserted = 0;
    let trans = conn.transaction()?;
    for ent in data.tiploc_data {
        if ent.contains_data() {
            ent.insert_self(&trans)?;
            inserted += 1;
        }
    }
    trans.commit()?;
    debug!("corpus_entries: inserted {} entries", inserted);
    Ok(())
}
fn apply_schedule_record<T: GenericConnection>(conn: &T, rec: ScheduleRecord, metaseq: i32) -> Result<()> {
    use ntrod_types::schedule::*;
    use ntrod_types::schedule::LocationRecord::*;
    match rec {
        ScheduleRecord::Delete { train_uid, schedule_start_date, stp_indicator, ..} => {
            debug!("apply_schedule_record: deleting schedules (UID {}, start {}, stp_indicator {:?})",
            train_uid, schedule_start_date, stp_indicator);
            conn.execute("DELETE FROM schedules
                          WHERE uid = $1 AND start_date = $2 AND stp_indicator = $3 AND source = 0",
                          &[&train_uid, &schedule_start_date.naive_utc(), &stp_indicator])?;
            Ok(())
        },
        ScheduleRecord::Create {
            train_uid,
            schedule_start_date,
            schedule_end_date,
            schedule_days_runs,
            stp_indicator,
            schedule_segment,
            ..
        } => {
            debug!("apply_schedule_record: inserting record (UID {}, start {}, stp_indicator {:?})",
            train_uid, schedule_start_date, stp_indicator);
            let ScheduleSegment {
                schedule_location,
                signalling_id,
                ..
            } = schedule_segment;
            let sched = Schedule {
                uid: train_uid.clone(),
                start_date: schedule_start_date.naive_utc(),
                end_date: schedule_end_date.naive_utc(),
                days: schedule_days_runs,
                stp_indicator,
                signalling_id,
                source: 0,
                file_metaseq: Some(metaseq),
                geo_generation: 0,
                id: -1,
                darwin_id: None
            };
            let (sid, updated) = sched.insert_self(conn)?;
            let mut mvts = vec![];
            for loc in schedule_location {
                match loc {
                    Originating { tiploc_code, departure, .. } => {
                        mvts.push((tiploc_code, departure, 1, true));
                    },
                    Intermediate { tiploc_code, arrival, departure, .. } => {
                        mvts.push((tiploc_code.clone(), arrival, 0, false));
                        mvts.push((tiploc_code, departure, 1, false));
                    },
                    Pass { tiploc_code, pass, .. } => {
                        mvts.push((tiploc_code, pass, 2, false));
                    },
                    Terminating { tiploc_code, arrival, .. } => {
                        mvts.push((tiploc_code, arrival, 0, true));
                    }
                }
            }
            if updated {
                warn!("apply_schedule_record: duplicate record (UID {}, start {}, stp_indicator {:?})",
                train_uid, schedule_start_date, stp_indicator);
                let orig_mvts = ScheduleMvt::from_select(conn, "WHERE parent_sched = $1 ORDER BY time ASC", &[&sid])?;
                let mut valid = true;
                if orig_mvts.len() != mvts.len() {
                    warn!("apply_schedule_record: invalidating prior schedule movements due to length difference");
                    valid = false;
                }
                else {
                    mvts.sort_by_key(|&(_, time, _, _)| time);
                    for (mvt, &(ref tiploc, time, action, origterm)) in orig_mvts.iter().zip(mvts.iter()) {
                        if mvt.tiploc == tiploc as &str && mvt.time == time && mvt.action == action && mvt.origterm == origterm {
                            trace!("apply_schedule_record: mvt #{} matches", mvt.id);
                        }
                        else {
                            warn!("apply_schedule_record: invalidating prior schedule movements due to mvt #{} mismatch", mvt.id);
                            valid = false;
                            break;
                        }
                    }
                }
                if valid {
                    debug!("apply_schedule_record: left movements untouched");
                    return Ok(());
                }
                else {
                    conn.execute("DELETE FROM schedule_movements WHERE parent_sched = $1", &[&sid])?;
                }
            }
            for (tiploc, time, action, origterm) in mvts {
                let mvt = ScheduleMvt {
                    parent_sched: sid,
                    id: -1,
                    starts_path: None,
                    ends_path: None,
                    tiploc, time, action, origterm
                };
                mvt.insert_self(conn)?;
            }
            Ok(())
        }
    }
}
pub fn apply_schedule_records<R: Read, T: GenericConnection>(conn: &T, file: R) -> Result<()> {
    debug!("apply_schedule_records: running...");
    let mut inserted = 0;
    let trans = conn.transaction()?;
    let rdr = BufReader::new(file);
    let mut metaseq = None;
    for line in rdr.lines() {
        let line = line?;
        let rec: ::std::result::Result<schedule::Record, _> = ::serde_json::from_str(&line);
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
                if let Some(ms) = metaseq {
                    apply_schedule_record(&trans, rec, ms)?;
                    inserted += 1;
                }
                else {
                    error!("apply_schedule_records: file contained no Timetable record!");
                    return Err(OsmsError::InvalidScheduleFile.into());
                }
            },
            schedule::Record::Timetable(rec) => {
                debug!("apply_schedule_records: this is a {}-type timetable (seq {}) from {} (ts: {})",
                       rec.metadata.ty, rec.metadata.sequence, rec.owner, rec.timestamp);
                debug!("apply_schedule_records: checking whether this timetable is new...");
                let files = ScheduleFile::from_select(&trans, "WHERE timestamp = $1", &[&(rec.timestamp as i64)])?;
                if files.len() > 0 {
                    error!("apply_schedule_records: schedule inserted already!");
                    return Err(OsmsError::ScheduleFileExists.into());
                }
                let full = ScheduleFile::from_select(&trans, "WHERE metaseq > $1", &[&(rec.metadata.sequence as i32)])?;
                if full.len() > 0 && rec.metadata.ty == "full" {
                    error!("apply_schedule_records: a schedule with a greater sequence number has been inserted!");
                    return Err(OsmsError::ScheduleFileImportInvalid("sequence number error").into());
                }
                debug!("apply_schedule_records: inserting file record...");
                let file = ScheduleFile {
                    id: -1,
                    timestamp: rec.timestamp as _,
                    metatype: rec.metadata.ty.clone(),
                    metaseq: rec.metadata.sequence as _
                };
                metaseq = Some(rec.metadata.sequence as i32);
                file.insert_self(&trans)?;
                debug!("apply_schedule_records: timetable OK");
            },
            _ => {}
        }
    }
    trans.execute("NOTIFY osms_schedule_updates;", &[])?;
    trans.commit()?;
    debug!("apply_schedule_records: applied {} entries", inserted);
    Ok(())
}
pub fn geo_process_schedules(pool: &DbPool, n_threads: usize) -> Result<()> {
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;
    use osms_db::osm;

    let n_schedules = util::count(&*pool.get().unwrap(), "FROM schedules WHERE geo_generation = 0", &[])?;
    let mut threads: Vec<::std::thread::JoinHandle<()>> = vec![];
    let processed = Arc::new(AtomicUsize::new(0));
    debug!("geo_process_schedules: {} schedules to process", n_schedules);
    for n in 0..n_threads {
        let pool = pool.clone();
        let p = processed.clone();
        debug!("geo_process_schedules: starting thread {}", n);
        threads.push(::std::thread::spawn(move || {
            let conn = pool.get().unwrap();
            loop {
                let conn2 = pool.get().unwrap();
                let trans = conn.transaction().unwrap();
                let scheds = Schedule::from_select(&trans, "WHERE geo_generation = 0 FOR UPDATE SKIP LOCKED LIMIT 1", &[]).unwrap();
                let sched = match scheds.into_iter().nth(0) {
                    Some(s) => s,
                    None => break
                };
                let id = sched.id;
                osm::geo_process_schedule(&*conn2, sched).unwrap();
                trans.execute("UPDATE schedules SET geo_generation = 1 WHERE id = $1", &[&id]).unwrap();
                trans.commit().unwrap();
                let cnt = p.fetch_add(1, Ordering::Relaxed);
                debug!("geo_process_schedules: {} of {} schedules processed ({:.02}%)", cnt, n_schedules, ((cnt as f32 / n_schedules as f32) * 100.0f32));
            }
            debug!("geo_process_schedules: thread {} done", n);
        }));
    }
    for thr in threads {
        thr.join().map_err(|_| format_err!("thread failed"))?;
    }
    Ok(())
}
pub fn naptan_entries<R: Read, T: GenericConnection>(conn: &T, file: R) -> Result<()> {
    let trans = conn.transaction()?;
    info!("naptan_entries: Importing naptan entries...");
    let mut rdr = ::csv::Reader::from_reader(file);
    for result in rdr.deserialize() {
        let rec: NaptanCsv = result?;
        let mut pgp: Option<PgPoint> = None;
        for row in &trans.query("SELECT ST_Transform(ST_SetSRID(ST_MakePoint($1, $2), 27700), 4326)", &[&(rec.easting as f64), &(rec.northing as f64)])? {
            pgp = Some(row.get(0));
        }
        let pgp = pgp.ok_or(format_err!("couldn't transform point"))?;
        let npt = NaptanEntry {
            atco: rec.atcocode,
            tiploc: rec.tiploccode,
            crs: rec.crscode,
            name: rec.stationname,
            loc: pgp
        };
        npt.insert_self(&trans)?;
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
    use geo::algorithm::from_postgis::FromPostgis;
    use geo::algorithm::to_postgis::ToPostgis;
    use crossbeam::deque;

    if ctx.count("FROM links")? != 0 { return Ok(()) };
    info!("Phase 1.2: making links");

    let bar = ctx.make_bar();
    bar.set_message("Beginning link import");
    let (worker, stealer) = deque::fifo();

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
                let db = pool.get().unwrap();
                'outer: loop {
                    while let Some(way) = stealer.steal() {
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
                                way: ls.to_postgis_wgs84(),
                                distance: dist as _
                            };
                            link.insert(&trans).unwrap();
                        }
                        trans.commit().unwrap();
                        bar.inc(1);
                    }
                    if stealer.is_empty() {
                        debug!("links: thread done");
                        bar.finish();
                        break;
                    }
                }
            });
        }
    });
    Ok(())
}
pub fn stations<R: Read + Seek>(ctx: &mut ImportContext<R>) -> Result<()> {
    use geo::algorithm::haversine_destination::HaversineDestination;
    use geo::algorithm::from_postgis::FromPostgis;
    use geo::algorithm::to_postgis::ToPostgis;


    if ctx.count("FROM stations")? != 0 { return Ok(()) };
    info!("Making stations...");
    let conn = ctx.get_conn();
    let trans = conn.transaction()?;
    let naptan_entries = NaptanEntry::from_select(&trans, "", &[])?;
    let bar = ctx.make_custom_bar(naptan_entries.len() as _);
    let mut polys: HashMap<String, Polygon<f64>> = HashMap::new();
    bar.set_message("Processing NAPTAN entries...");
    for ent in naptan_entries {
        bar.set_message(&format!("Processing naptan for {}", ent.tiploc));
        let entries = CorpusEntry::from_select(&trans, "WHERE tiploc = $1 AND stanox IS NOT NULL", &[&ent.tiploc])?;
        if entries.len() != 1 {
            warn!("TIPLOC {} doesn't map to one single STANOX\n", ent.tiploc);
            continue;
        }
        let nr_ref = entries.into_iter().nth(0).unwrap().stanox.unwrap();
        let pt = Point::from_postgis(&ent.loc);
        let mut nodes = vec![];
        for bearing in 0..360 {
            nodes.push(pt.haversine_destination(bearing as _, 50.0));
        }
        let nd = nodes[0];
        nodes.push(nd);
        let mut poly = Polygon { exterior: LineString(nodes), interiors: vec![] };
        if let Some(p2) = polys.get(&nr_ref) {
            let p1 = poly.to_postgis_wgs84();
            let p2 = p2.to_postgis_wgs84(); 
            for row in &trans.query("SELECT ST_ConvexHull(ST_Collect($1, $2));", &[&p1, &p2])? {
                let p: PgPolygon = row.get(0);
                poly = Option::from_postgis(&p).unwrap();
            }
        }
        polys.insert(nr_ref, poly);
        bar.inc(1);
    }
    bar.finish();
    let bar = ctx.make_custom_bar(polys.len() as _);
    bar.set_message("Making stations...");
    for (nr_ref, poly) in polys {
        use osms_db::osm::{self, ProcessStationError};

        bar.set_message(&format!("Processing station for {}", nr_ref));
        match osm::process_station(&trans, &nr_ref, poly) {
            Ok(_) => {},
            Err(ProcessStationError::AlreadyProcessed) => {
                debug!("Already processed station for {}\n", nr_ref);
            },
            Err(ProcessStationError::Problematic(poly, code)) => {
                warn!("Station {} is problematic: code {}\n", nr_ref, code);
                ProblematicStation::insert(&trans, &nr_ref, poly, code)?;
            },
            Err(ProcessStationError::Error(e)) => {
                Err(e)?
            }
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
