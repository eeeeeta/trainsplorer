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
/// A single location on the railway, referenced by various identifying codes.
#[derive(Debug, Clone)]
pub struct RailwayLocation {
    /// Internal ID.
    pub id: i32,
    /// The public-facing name of this location (e.g. "Clapham Junction"),
    /// as might be displayed to passengers.
    pub name: String,
    /// References a `Node` giving the approximate center of this location.
    pub point: i64,
    /// The location's area. This just needs to cut through some tracks - it
    /// doesn't actually represent, e.g. the area of a station building.
    pub area: Polygon,
    /// The STANOX of this location, if it has one.
    pub stanox: Option<String>,
    /// The TIPLOCs covered by this location entry, if any.
    pub tiploc: Vec<String>,
    /// The CRS codes covered by this location entry, if any.
    pub crs: Vec<String>,
    /// An optional problem identified with this location.
    pub defect: Option<i32>
}
impl DbType for RailwayLocation {
    fn table_name() -> &'static str {
        "railway_locations"
    }
    fn from_row(row: &Row) -> Self {
        Self {
            id: row.get(0),
            name: row.get(1),
            point: row.get(2),
            area: row.get(3),
            stanox: row.get(4),
            tiploc: row.get(5),
            crs: row.get(6),
            defect: row.get(7),
        }
    }
}
impl InsertableDbType for RailwayLocation {
    type Id = i32;
    fn insert_self<T: GenericConnection>(&self, conn: &T) -> Result<i32> {
        let qry = conn.query("INSERT INTO railway_locations
                              (name, point, area, stanox, tiploc, crs, defect)
                              VALUES ($1, $2, $3, $4, $5, $6, $7)
                              RETURNING id",
                             &[&self.name, &self.point, &self.area, &self.stanox,
                               &self.tiploc, &self.crs, &self.defect])?;
        let mut ret = None;
        for row in &qry {
            ret = Some(row.get(0))
        }
        Ok(ret.expect("no ID in RailwayLocation insert"))
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
    pub id: i32,
    pub s1: i32,
    pub s2: i32,
    pub way: LineString,
    pub nodes: Vec<i64>,
}
impl DbType for StationPath {
    fn table_name() -> &'static str {
        "station_paths"
    }
    fn from_row(row: &Row) -> Self {
        Self {
            id: row.get(0),
            s1: row.get(1),
            s2: row.get(2),
            way: row.get(3),
            nodes: row.get(4),
        }
    }
}
impl InsertableDbType for StationPath {
    type Id = i32;
    fn insert_self<T: GenericConnection>(&self, conn: &T) -> Result<i32> {
        let qry = conn.query("INSERT INTO station_paths
                              (s1, s2, way, nodes)
                              VALUES ($1, $2, $3, $4)
                              ON CONFLICT(s1, s2) DO UPDATE
                                 SET way = excluded.way, nodes = excluded.nodes
                              RETURNING id",
                             &[&self.s1, &self.s2, &self.way, &self.nodes])?;
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
