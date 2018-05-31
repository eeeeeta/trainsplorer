pub mod types;
pub mod navigate;
pub mod org;

use db::*;
use self::types::*;
use geo::*;
use std::collections::HashSet;
use postgis::ewkb::Polygon as PgPolygon;

pub enum ProcessStationError {
    AlreadyProcessed,
    Problematic(PgPolygon, i32),
    Error(::failure::Error)
}
impl<T> From<T> for ProcessStationError where T: Into<::failure::Error> {
    fn from(ty: T) -> Self {
        ProcessStationError::Error(ty.into())
    }
}

pub fn remove_station<T: GenericConnection>(conn: &T, nr_ref: &str) -> Result<(), ::failure::Error> {
    let stations = Station::from_select(conn, "WHERE nr_ref = $1", &[&nr_ref])?;
    for sta in stations {
        conn.execute("DELETE FROM nodes WHERE id = $1", &[&sta.point])?;
    }
    Ok(())
}
pub fn process_station<T: GenericConnection>(conn: &T, nr_ref: &str, poly: Polygon<f64>) -> Result<i32, ProcessStationError> {
    use geo::algorithm::haversine_length::HaversineLength;
    use geo::algorithm::centroid::Centroid;
    use geo::algorithm::from_postgis::FromPostgis;
    use geo::algorithm::to_postgis::ToPostgis;

    let sta = Station::from_select(conn, "WHERE nr_ref = $1", &[&nr_ref])?;
    if sta.len() > 0 {
        debug!("Station for {} already exists", nr_ref);
        return Err(ProcessStationError::AlreadyProcessed);
    }
    let pgpoly = poly.to_postgis_wgs84();
    let lks = Link::from_select(conn, "WHERE ST_Intersects(way, $1)", &[&pgpoly])?;
    if lks.len() == 0 {
        debug!("Polygon for {} doesn't connect to anything", nr_ref);
        return Err(ProcessStationError::Problematic(pgpoly, 0));
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
    let ret = Station {
        id: -1,
        nr_ref: nr_ref.into(),
        point: nd,
        area: pgpoly
    }.insert_self(conn)?;
    Ok(ret)
}
