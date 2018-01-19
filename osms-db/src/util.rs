use postgres::GenericConnection;
use postgres::types::ToSql;
use errors::*;
use geo;
use postgis::ewkb::{Point, LineString, Polygon};


pub fn count<T: GenericConnection>(conn: &T, details: &str, args: &[&ToSql]) -> Result<i64> {
    Ok(conn.query(&format!("SELECT COUNT(*) {}", details), args)?.into_iter()
       .nth(0)
       .ok_or(OsmsError::ExtraterrestrialActivity)?
       .get(0))
}
pub fn geo_bbox_to_poly(bbox: geo::Bbox<f64>) -> geo::Polygon<f64> {
    geo::Polygon {
        exterior: geo::LineString(
                      vec![
                      geo::Point::new(bbox.xmin, bbox.ymin),
                      geo::Point::new(bbox.xmin, bbox.ymax),
                      geo::Point::new(bbox.xmax, bbox.ymax),
                      geo::Point::new(bbox.xmax, bbox.ymin),
                      geo::Point::new(bbox.xmin, bbox.ymin),
                      ]),
        interiors: vec![]
    }
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
pub fn geo_poly_to_postgis(poly: geo::Polygon<f64>) -> Polygon {
    let rings = ::std::iter::once(poly.exterior)
        .chain(poly.interiors.into_iter())
        .map(geo_ls_to_postgis)
        .collect::<Vec<_>>();
    Polygon {
        rings,
        srid: Some(4326)
    }
}
#[macro_export]
macro_rules! impl_from_for_error {
    ($error:ident, $($orig:ident => $var:ident),*) => {
        $(
            impl From<$orig> for $error {
                fn from(err: $orig) -> $error {
                    $error::$var(err)
                }
            }
        )*
    }
}
