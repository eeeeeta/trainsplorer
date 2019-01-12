pub mod types;
pub mod navigate;
pub mod org;

use db::*;
use self::types::*;
use ntrod::types::*;
use geo::*;
use std::collections::HashSet;

pub fn update_railway_location<T: GenericConnection>(conn: &T, id: i32, area: Polygon<f64>) -> Result<i32, ::failure::Error> {
    let trans = conn.transaction()?;
    let rlocs = RailwayLocation::from_select(conn, "WHERE id = $1", &[&id])?;
    let rloc = rlocs.into_iter().nth(0).ok_or(format_err!("No such location."))?;
    remove_railway_location(&trans, id)?;
    let new_id = process_railway_location(&trans, rloc.name.clone(), area)?;
    trans.execute("UPDATE railway_locations SET stanox = $1, tiploc = $2, crs = $3 WHERE id = $4",
                  &[&rloc.stanox, &rloc.tiploc, &rloc.crs, &new_id])?;
    trans.commit()?;
    Ok(new_id)
}
pub fn remove_railway_location<T: GenericConnection>(conn: &T, id: i32) -> Result<(), ::failure::Error> {
    let stations = RailwayLocation::from_select(conn, "WHERE id = $1", &[&id])?;
    for sta in stations {
        conn.execute("DELETE FROM nodes WHERE id = $1", &[&sta.point])?;
    }
    Ok(())
}
pub fn process_railway_location<T: GenericConnection>(conn: &T, name: String, poly: Polygon<f64>) -> Result<i32, ::failure::Error> {
    use geo::algorithm::haversine_length::HaversineLength;
    use geo::algorithm::centroid::Centroid;
    use geo::algorithm::from_postgis::FromPostgis;
    use geo::algorithm::to_postgis::ToPostgis;

    let pgpoly = poly.to_postgis_wgs84();
    let mut defect = None;
    let lks = Link::from_select(conn, "WHERE ST_Intersects(way, $1)", &[&pgpoly])?;
    if lks.len() == 0 {
        warn!("Polygon for new station '{}' doesn't connect to anything", name);
        defect = Some(1);
    }
    let centroid = poly.centroid().unwrap();
    let nd = Node::insert(conn, centroid.to_postgis_wgs84())?;
    let mut connected = HashSet::new();
    for link in lks {
        if link.p1 == link.p2 {
            continue;
        }
        if !connected.insert(link.p1) || !connected.insert(link.p2) {
            continue;
        }
        let pt1 = Node::from_select(conn, "WHERE id = $1", &[&link.p1])?
            .into_iter().nth(0).ok_or(format_err!("foreign key fail"))?;
        let pt2 = Node::from_select(conn, "WHERE id = $1", &[&link.p2])?
            .into_iter().nth(0).ok_or(format_err!("foreign key fail"))?;
        let lp1 = Point::from_postgis(&pt1.location);
        let lp2 = Point::from_postgis(&pt2.location);
        let lp1_station = LineString(vec![lp1, centroid.clone()]);
        let lp1_s_dist = lp1_station.haversine_length();
        let station_lp2 = LineString(vec![centroid.clone(), lp2]);
        let s_lp2_dist = station_lp2.haversine_length();
        Link {
            p1: link.p1,
            p2: nd,
            way: lp1_station.to_postgis_wgs84(),
            distance: lp1_s_dist as f32
        }.insert(conn)?;
        Link {
            p1: nd,
            p2: link.p2,
            way: station_lp2.to_postgis_wgs84(),
            distance: s_lp2_dist as f32
        }.insert(conn)?;
    }
    let ret = RailwayLocation {
        id: -1,
        name,
        point: nd,
        area: pgpoly,
        stanox: None,
        tiploc: vec![],
        crs: vec![],
        defect
    }.insert_self(conn)?;
    Ok(ret)
}
/// The maximum number of consecutive stations we can skip through
/// when searching for a valid route between two stations in `geo_process_schedule()`.
pub const SCHEDULE_MAX_ARRIVAL_SKIPS: usize = 2;
/// Links geodata to a given schedule, routing between adjacent stations where possible
/// and annotating the schedule's movements with `starts_path` and `ends_path` values.
pub fn geo_process_schedule<T: GenericConnection>(conn: &T, sched: Schedule) -> Result<(), ::failure::Error> {
    // step 1: get all the schedule's movements that have associated railway locations
    //
    // FIXME: this is currently borked if `time` jumps back to 0000 (e.g. for schedules spanning 2
    // days)
    // FIXME: we could theoretically JOIN here instead of using generic queries for a speedup.
    let mvts = ScheduleMvt::from_select(conn, "WHERE parent_sched = $1
                                               AND EXISTS(
                                                   SELECT * FROM railway_locations
                                                   WHERE schedule_movements.tiploc = ANY(railway_locations.tiploc))
                                               ORDER BY time ASC", &[&sched.id])?;
    debug!("geo_process_schedule: processing schedule {} ({} mvts)", sched.id, mvts.len());
    // step 2: loop through the movements, routing between adjacent stations, sort of
    let mut start_ptr; // current mvt we're trying to route _from_
    let mut end_ptr = 0; // current mvt we're trying to route _to_
    loop {
        // step one, make our start_ptr our old end_ptr
        start_ptr = end_ptr;
        // ...then search through mvts, starting at our start_ptr, for a valid departure or pass
        let mut new_start = None;
        for idx in start_ptr..mvts.len() {
            let mvt = &mvts[idx];
            if mvt.action == ScheduleMvt::ACTION_DEPARTURE || mvt.action == ScheduleMvt::ACTION_PASS {
                let station = RailwayLocation::from_select(conn, "WHERE $1 = ANY(tiploc)", &[&mvt.tiploc])?
                    .into_iter().nth(0).unwrap();
                new_start = Some((idx, station.id));
            }
        }
        if let Some((start_idx, start_stn)) = new_start {
            // begin searching from the next mvt along
            end_ptr = start_idx + 1;
            if end_ptr >= mvts.len() {
                // this shouldn't happen, since we should fail to find a start first.
                // indicates either a programming error or a wacky schedule.
                warn!("geo_process_schedule: end pointer overran for sched #{}", sched.id);
                break;
            }
            // step two, search through mvts, starting @ end_ptr, for a valid arrival or pass
            // if we find one, try and route to it; if we fail, we continue searching.
            let mut arrs_skipped = 0;
            for end_idx in end_ptr..mvts.len() {
                let start_mvt = &mvts[start_idx];
                let end_mvt = &mvts[end_idx];
                if end_mvt.action == ScheduleMvt::ACTION_ARRIVAL || end_mvt.action == ScheduleMvt::ACTION_PASS {
                    let end_stn = RailwayLocation::from_select(conn, "WHERE $1 = ANY(tiploc)", &[&end_mvt.tiploc])?
                        .into_iter().nth(0).unwrap();
                    let trans = conn.transaction()?;
                    debug!("geo_process_schedule: navigating from {} (mvt #{}) to {} (mvt #{})", start_stn, start_mvt.id, end_stn.id, end_mvt.id);
                    match navigate::navigate_cached(&trans, start_stn, end_stn.id) {
                        Ok(pid) => {
                            debug!("geo_process_schedule: navigation successful, got sp {} for {} and {}", pid, start_mvt.id, end_mvt.id);
                            trans.execute("UPDATE schedule_movements SET starts_path = $1 WHERE id = $2", &[&pid, &start_mvt.id])?;
                            trans.execute("UPDATE schedule_movements SET ends_path = $1 WHERE id = $2", &[&pid, &end_mvt.id])?;
                        },
                        Err(e) => {
                            warn!("geo_process_schedule: error navigating from #{} to #{}: {}", start_stn, end_stn.id, e);
                            if end_mvt.action == ScheduleMvt::ACTION_ARRIVAL {
                                // prevent skipping over too many arrivals; there might be
                                // something wrong with our start station!
                                // (passes are fine, because they're usually through junctions or
                                // something which we won't have locations for)
                                arrs_skipped += 1;
                                if arrs_skipped > SCHEDULE_MAX_ARRIVAL_SKIPS {
                                    warn!("geo_process_schedule: abandoning navigation from mvt #{}; too many arrivals skipped", start_mvt.id);
                                    break;
                                }
                            }
                            // skip over this end_mvt and get a new one
                            continue;
                        },
                    }
                    // navigation successful, loop back to start
                    trans.commit()?;
                    break;
                }
            }
        }
        else {
            // welp, we're done here
            break;
        }
    }
    Ok(())
}
