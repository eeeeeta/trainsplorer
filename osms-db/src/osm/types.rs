use postgis::ewkb::{Point, LineString, Polygon};
use db::{DbType, InsertableDbType, GenericConnection, Row};
use errors::*;

#[derive(Debug, Clone)]
pub struct Node {
    pub id: i64,
    pub location: Point,
    pub graph_part: i32,
    pub parent_crossing: Option<i32>,
    pub orig_osm_id: Option<i64>,
    pub osm_was_crossing: bool
}
impl DbType for Node {
    fn table_name() -> &'static str {
        "nodes"
    }
    fn from_row(row: &Row) -> Self {
        Self {
            id: row.get(0),
            location: row.get(1),
            graph_part: row.get(2),
            parent_crossing: row.get(3),
            orig_osm_id: row.get(4),
            osm_was_crossing: row.get(5),
        }
    }
}
impl InsertableDbType for Node {
    type Id = i64;
    fn insert_self<T: GenericConnection>(&self, conn: &T) -> Result<i64> {
        let qry = conn.query("INSERT INTO nodes
                              (location, graph_part, parent_crossing, orig_osm_id, osm_was_crossing)
                              VALUES ($1, $2, $3, $4, $5)
                              RETURNING id",
                             &[&self.location, &self.graph_part, &self.parent_crossing,
                               &self.orig_osm_id, &self.osm_was_crossing])?;
        let mut ret = None;
        for row in &qry {
            ret = Some(row.get(0))
        }
        Ok(ret.expect("no ID in Node insert"))
    }
}
impl Node {
    pub fn insert<T: GenericConnection>(conn: &T, location: Point) -> Result<i64> {
        let node = Node {
            id: -1,
            location,
            graph_part: 0,
            parent_crossing: None,
            orig_osm_id: None,
            osm_was_crossing: false
        };
        node.insert_self(conn)
    }
}
#[derive(Debug, Clone)]
pub struct Station {
    pub id: i32,
    pub nr_ref: String,
    pub point: i64,
    pub area: Polygon,
    pub name: String
}
impl DbType for Station {
    fn table_name() -> &'static str {
        "stations"
    }
    fn from_row(row: &Row) -> Self {
        Self {
            id: row.get(0),
            nr_ref: row.get(1),
            point: row.get(2),
            area: row.get(3),
            name: row.get(4)
        }
    }
}
impl InsertableDbType for Station {
    type Id = i32;
    fn insert_self<T: GenericConnection>(&self, conn: &T) -> Result<i32> {
        let qry = conn.query("INSERT INTO stations
                              (nr_ref, point, area, name)
                              VALUES ($1, $2, $3, $4)
                              RETURNING id",
                             &[&self.nr_ref, &self.point, &self.area, &self.name])?;
        let mut ret = None;
        for row in &qry {
            ret = Some(row.get(0))
        }
        Ok(ret.expect("no ID in Station insert"))
    }
}
pub struct StationOverride {
    pub id: i32,
    pub nr_ref: String,
    pub area: Polygon
}
impl DbType for StationOverride {
    fn table_name() -> &'static str {
        "station_overrides"
    }
    fn from_row(row: &Row) -> Self {
        Self {
            id: row.get(0),
            nr_ref: row.get(1),
            area: row.get(2),
        }
    }
}
impl StationOverride {
    pub fn insert<T: GenericConnection>(conn: &T, nr_ref: &str, area: Polygon) -> Result<()> {
        conn.execute("INSERT INTO station_overrides (nr_ref, area) VALUES ($1, $2)
                      ON CONFLICT DO UPDATE",
                     &[&nr_ref, &area])?;
        Ok(())
    }
}
pub struct StationNavigationProblem {
    pub id: i32,
    pub geo_generation: i32,
    pub origin: i32,
    pub destination: i32,
    pub desc: String
}
impl DbType for StationNavigationProblem {
    fn table_name() -> &'static str {
        "station_navigation_problems"
    }
    fn from_row(row: &Row) -> Self {
        Self {
            id: row.get(0),
            geo_generation: row.get(1),
            origin: row.get(2),
            destination: row.get(3),
            desc: row.get(4),
        }
    }
}
impl StationNavigationProblem {
    pub fn insert<T: GenericConnection>(conn: &T, gen: i32, orig: i32, dest: i32, desc: String) -> Result<()> {
        conn.execute("INSERT INTO station_navigation_problems
                     (geo_generation, origin, destination, descrip)
                     VALUES ($1, $2, $3, $4)
                     ON CONFLICT DO NOTHING",
                     &[&gen, &orig, &dest, &desc])?;
        Ok(())
    }
}

pub struct ProblematicStation {
    pub id: i32,
    pub nr_ref: String,
    pub area: Polygon,
    pub defect: i32
}
impl DbType for ProblematicStation {
    fn table_name() -> &'static str {
        "problematic_stations"
    }
    fn from_row(row: &Row) -> Self {
        Self {
            id: row.get(0),
            nr_ref: row.get(1),
            area: row.get(2),
            defect: row.get(3),
        }
    }
}
impl ProblematicStation {
    pub fn insert<T: GenericConnection>(conn: &T, nr_ref: &str, area: Polygon, defect: i32) -> Result<()> {
        conn.execute("INSERT INTO problematic_stations (nr_ref, area, defect) VALUES ($1, $2, $3)",
                     &[&nr_ref, &area, &defect])?;
        Ok(())
    }
}
#[derive(Debug, Clone)]
pub struct Link {
    pub p1: i64,
    pub p2: i64,
    pub way: LineString,
    pub distance: f32
}
impl DbType for Link {
    fn table_name() -> &'static str {
        "links"
    }
    fn from_row(row: &Row) -> Self {
        Self {
            p1: row.get(0),
            p2: row.get(1),
            way: row.get(2),
            distance: row.get(3)
        }
    }
}
impl Link {
    pub fn insert<T: GenericConnection>(&self, conn: &T) -> Result<()> {
        conn.execute("INSERT INTO links (p1, p2, way, distance) VALUES ($1, $2, $3, $4)
                      ON CONFLICT DO NOTHING",
                     &[&self.p1, &self.p2, &self.way, &self.distance])?;
        Ok(())
    }
}
#[derive(Debug, Clone)]
pub struct StationPath {
    pub s1: i32,
    pub s2: i32,
    pub way: LineString,
    pub nodes: Vec<i64>,
    pub crossings: Vec<i32>,
    pub crossing_locations: Vec<f64>,
    pub id: i32
}
impl DbType for StationPath {
    fn table_name() -> &'static str {
        "station_paths"
    }
    fn from_row(row: &Row) -> Self {
        Self {
            s1: row.get(0),
            s2: row.get(1),
            way: row.get(2),
            nodes: row.get(3),
            crossings: row.get(4),
            crossing_locations: row.get(5),
            id: row.get(6)
        }
    }
}
impl InsertableDbType for StationPath {
    type Id = i32;
    fn insert_self<T: GenericConnection>(&self, conn: &T) -> Result<i32> {
        let qry = conn.query("INSERT INTO station_paths
                              (s1, s2, way, nodes, crossings, crossing_locations)
                              VALUES ($1, $2, $3, $4, $5, $6)
                              ON CONFLICT(s1, s2) DO UPDATE SET way = excluded.way
                              RETURNING id",
                             &[&self.s1, &self.s2, &self.way, &self.nodes,
                               &self.crossings, &self.crossing_locations])?;
        let mut ret = None;
        for row in &qry {
            ret = Some(row.get(0))
        }
        Ok(ret.expect("no ID in StationPath insert"))
    }
}
pub struct Crossing {
    pub id: i32,
    pub name: Option<String>,
    pub area: Polygon
}
impl DbType for Crossing {
    fn table_name() -> &'static str {
        "crossings"
    }
    fn from_row(row: &Row) -> Self {
        Self {
            id: row.get(0),
            name: row.get(1),
            area: row.get(2)
        }
    }
}
impl Crossing {
    pub fn insert<T: GenericConnection>(conn: &T, name: Option<String>, area: Polygon) -> Result<i32> {
        let qry = conn.query("INSERT INTO crossings
                              (name, area)
                              VALUES ($1, $2)
                              RETURNING id",
                             &[&name, &area])?;
        let mut ret = None;
        for row in &qry {
            ret = Some(row.get(0))
        }
        Ok(ret.expect("No id in Crossing::insert?!"))
    }
}
