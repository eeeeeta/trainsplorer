use postgres::GenericConnection;
use postgres::types::ToSql;
use errors::*;
use geo;
use postgis::ewkb::{Point, LineString};


pub fn count<T: GenericConnection>(conn: &T, details: &str, args: &[&ToSql]) -> Result<i64> {
    Ok(conn.query(&format!("SELECT COUNT(*) {}", details), args)?.into_iter()
       .nth(0)
       .ok_or("Count query failed")?
       .get(0))
}
pub fn geo_pt_to_postgis(pt: geo::Point<f64>) -> Point {
    Point::new(pt.0.x, pt.0.y, Some(4326))
}
pub fn geo_ls_to_postgis(ls: geo::LineString<f64>) -> LineString {
    LineString {
        points: ls.0.into_iter().map(geo_pt_to_postgis).collect(),
        srid: Some(4326)
    }
}
