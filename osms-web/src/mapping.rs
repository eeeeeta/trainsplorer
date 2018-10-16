use geojson::{Feature, FeatureCollection};
use geo::*;
use sctx::Sctx;
use postgis::ewkb::Polygon as PgPolygon;
use osms_db::db::*;
use osms_db::util;
use osms_db::osm;
use osms_db::osm::types::*;
use super::Result;
use geojson::conversion;
use geo::algorithm::from_postgis::FromPostgis;
use rouille::{Request, Response};
use tmpl::TemplateContext;

struct GeoParameters {
    xmin: f64,
    xmax: f64,
    ymin: f64,
    ymax: f64,
    limit: Option<u32>
}
impl GeoParameters {
    fn get_from_req(req: &Request) -> Result<Self> {
        let xmin = req.get_param("xmin")
            .ok_or(format_err!("xmin not provided"))?.parse()?;
        let xmax = req.get_param("xmax")
            .ok_or(format_err!("xmax not provided"))?.parse()?;
        let ymin = req.get_param("ymin")
            .ok_or(format_err!("ymin not provided"))?.parse()?;
        let ymax = req.get_param("ymax")
            .ok_or(format_err!("ymax not provided"))?.parse()?;
        let limit = if let Some(l) = req.get_param("limit") {
            Some(l.parse()?)
        }
        else {
            None
        };
        Ok(Self {
            xmax, xmin, ymin, ymax, limit
        })
    }
    fn as_bbox(&self) -> Bbox<f64> {
        Bbox {
            xmin: self.xmin,
            xmax: self.xmax,
            ymin: self.ymin,
            ymax: self.ymax
        }
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
pub fn map(sctx: Sctx) -> Response {
    render!(sctx, TemplateContext::title("map", "Slippy map"))
}
pub fn geo_correct_station(sctx: Sctx, req: &Request) -> Response {
    use geojson::conversion::TryInto;
    use geo::algorithm::to_postgis::ToPostgis;

    let db = get_db!(sctx);
    let details: CorrectionDetails = try_or_badreq!(sctx, ::rouille::input::json::json_input(req));
    let poly = try_or_badreq!(sctx, details.poly.geometry.ok_or(format_err!("no geometry provided")));
    let mut poly: Polygon<f64> = try_or_badreq!(sctx, poly.value.try_into());
    // FIXME: postgis complains if you have an empty ring; this should
    // ideally be fixed in the geo library
    poly.interiors = vec![];
    let pgpoly = poly.to_postgis_wgs84();
    try_or_ise!(sctx, StationOverride::insert(&*db, &details.name, pgpoly));
    try_or_ise!(sctx, osm::remove_station(&*db, &details.name));
    match osm::process_station(&*db, &details.name, poly) {
        Ok(_) => {},
        Err(osm::ProcessStationError::AlreadyProcessed) => {
            try_or_ise!(sctx, Err(format_err!("already processed, somehow")));
        },
        Err(osm::ProcessStationError::Problematic(_, id)) => {
            warn!("FIXME: it was problematic: {}", id);
        },
        Err(osm::ProcessStationError::Error(e)) => {
            try_or_ise!(sctx, Err(e));
        }
    }
    Response::empty_204()
}
pub fn geo_stations(sctx: Sctx, req: &Request) -> Response {
    let db = get_db!(sctx);
    let geo = try_or_badreq!(sctx, GeoParameters::get_from_req(req));
    let (limit, poly) = geo.get_limit_and_pg_poly();
    let mut features = vec![];
    let stn_query = Station::from_select(&*db, "WHERE ST_Intersects(area, $1) LIMIT $2", &[&poly, &(limit as i64)]);
    for stn in try_or_ise!(sctx, stn_query) {
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
    Response::json(&FeatureCollection {
        bbox: None,
        features,
        foreign_members: None
    })
}
pub fn geo_ways(sctx: Sctx, req: &Request) -> Response {
    let db = get_db!(sctx);
    let geo = try_or_badreq!(sctx, GeoParameters::get_from_req(req));
    let (limit, poly) = geo.get_limit_and_pg_poly();
    let mut features = vec![];
    let link_query = Link::from_select(&*db, "WHERE ST_Intersects(way, $1) LIMIT $2", &[&poly, &(limit as i64)]);
    for link in try_or_ise!(sctx, link_query) {
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
    Response::json(&FeatureCollection {
        bbox: None,
        features,
        foreign_members: None
    })
}
