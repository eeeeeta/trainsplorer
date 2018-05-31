use geojson::{Feature, FeatureCollection};
use geo::*;
use postgis::ewkb::Polygon as PgPolygon;
use osms_db::db::*;
use osms_db::util;
use osms_db::osm;
use osms_db::osm::types::*;
use pool::DbConn;
use super::Result;
use rocket_contrib::Json;
use geojson::conversion;
use geo::algorithm::from_postgis::FromPostgis;

#[derive(FromForm)]
struct GeoParameters {
    xmin: f64,
    xmax: f64,
    ymin: f64,
    ymax: f64,
    limit: Option<u32>
}
impl GeoParameters {
    fn as_bbox(&self) -> Bbox<f64> {
        Bbox {
            xmin: self.xmin,
            xmax: self.xmax,
            ymin: self.ymin,
            ymax: self.ymax
        }
    }
    fn area(&self) -> f64 {
        use geo::algorithm::area::Area;
        Area::area(&self.as_bbox())
    }
    fn as_poly(&self) -> Polygon<f64> {
        util::geo_bbox_to_poly(self.as_bbox())
    }
    fn as_pg_poly(&self) -> PgPolygon {
        use geo::algorithm::to_postgis::ToPostgis;
        self.as_poly().to_postgis_wgs84()
    }
    fn get_limit_and_pg_poly(&self) -> (u32, PgPolygon) {
        let mut limit = self.limit.unwrap_or(1000);
        if limit > 1000 {
            limit = 1000;
        }
        (limit, self.as_pg_poly())
    }
}
#[derive(Deserialize)]
struct CorrectionDetails {
    name: String,
    poly: Feature
}
#[post("/geo/correct_station", data = "<details>")]
fn geo_correct_station(db: DbConn, details: Json<CorrectionDetails>) -> Result<()> {
    use geojson::conversion::TryInto;

    let poly = details.0.poly.geometry.ok_or(format_err!("no geometry provided"))?;
    let mut poly: Polygon<f64> = poly.value.try_into()?;
    println!("poly: {:?}", poly);
    // FIXME: postgis complains if you have an empty ring; this should
    // ideally be fixed in the geo library
    poly.interiors = vec![];
    osm::remove_station(&*db, &details.0.name)?;
    match osm::process_station(&*db, &details.0.name, poly) {
        Ok(_) => {},
        Err(osm::ProcessStationError::AlreadyProcessed) => {
            Err(format_err!("already processed, somehow"))?;
        },
        Err(osm::ProcessStationError::Problematic(_, id)) => {
            eprintln!("FIXME: it was problematic: {}", id);
        },
        Err(osm::ProcessStationError::Error(e)) => {
            Err(e)?;
        }
    }
    Ok(())
}
#[get("/geo/stations?<geo>")]
fn geo_stations(db: DbConn, geo: GeoParameters) -> Result<Json<FeatureCollection>> {
    let (limit, poly) = geo.get_limit_and_pg_poly();
    let mut features = vec![];
    for stn in Station::from_select(&*db, "WHERE ST_Intersects(area, $1) LIMIT $2", &[&poly, &(limit as i64)])? {
        let mut ls = vec![];
        for point in stn.area.rings[0].points.iter() {
            ls.push(vec![point.x, point.y]);
        }
        let geom = ::geojson::Geometry {
            bbox: None,
            value: ::geojson::Value::Polygon(vec![ls]),
            foreign_members: None
        };
        let mut props = ::serde_json::Map::new();
        props.insert("nr_ref".to_string(), json!(stn.nr_ref));
        features.push(Feature {
            properties: Some(props),
            bbox: None,
            geometry: Some(geom),
            id: None,
            foreign_members: None
        });
    }
    Ok(Json(FeatureCollection {
        bbox: None,
        features,
        foreign_members: None
    }))
}
#[get("/geo/ways?<geo>")]
fn geo_ways(db: DbConn, geo: GeoParameters) -> Result<Json<FeatureCollection>> {
    let (limit, poly) = geo.get_limit_and_pg_poly();
    let mut features = vec![];
    for link in Link::from_select(&*db, "WHERE ST_Intersects(way, $1) LIMIT $2", &[&poly, &(limit as i64)])? {
        let way = LineString::from_postgis(&link.way);
        let ls = conversion::create_line_string_type(&way);
        let geom = ::geojson::Geometry {
            bbox: None,
            value: ::geojson::Value::LineString(ls),
            foreign_members: None
        };
        let mut props = ::serde_json::Map::new();
        props.insert("p1".to_string(), json!(link.p1));
        props.insert("p2".to_string(), json!(link.p2));
        features.push(Feature {
            properties: Some(props),
            bbox: None,
            geometry: Some(geom),
            id: None,
            foreign_members: None
        });
    }
    Ok(Json(FeatureCollection {
        bbox: None,
        features,
        foreign_members: None
    }))
}
