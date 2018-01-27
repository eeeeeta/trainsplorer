use postgres::GenericConnection;
use postgres::types::ToSql;
use errors::*;
use geo;

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
