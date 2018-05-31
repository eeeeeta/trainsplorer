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
    fn table_desc() -> &'static str {
        r#"
id BIGSERIAL PRIMARY KEY,
location geometry NOT NULL,
graph_part INT NOT NULL DEFAULT 0,
parent_crossing INT REFERENCES crossings ON DELETE RESTRICT,
orig_osm_id BIGINT,
osm_was_crossing BOOL NOT NULL DEFAULT false
"#
    }
    fn indexes() -> Vec<&'static str> {
        vec![
            "nodes_id ON nodes (id)",
            "nodes_location ON nodes (location)",
            "nodes_geom ON nodes USING GIST (location)",
            "nodes_orig_osm_id ON nodes (orig_osm_id)"
        ]
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
    pub area: Polygon
}
impl DbType for Station {
    fn table_name() -> &'static str {
        "stations"
    }
    fn table_desc() -> &'static str {
        r#"
id SERIAL PRIMARY KEY,
nr_ref VARCHAR NOT NULL,
point BIGINT NOT NULL REFERENCES nodes ON DELETE CASCADE,
area geometry NOT NULL
"#
    }
    fn from_row(row: &Row) -> Self {
        Self {
            id: row.get(0),
            nr_ref: row.get(1),
            point: row.get(2),
            area: row.get(3),
        }
    }
}
impl InsertableDbType for Station {
    type Id = i32;
    fn insert_self<T: GenericConnection>(&self, conn: &T) -> Result<i32> {
        let qry = conn.query("INSERT INTO stations
                              (nr_ref, point, area)
                              VALUES ($1, $2, $3)
                              RETURNING id",
                             &[&self.nr_ref, &self.point, &self.area])?;
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
    fn table_desc() -> &'static str {
        r#"
id SERIAL PRIMARY KEY,
nr_ref VARCHAR NOT NULL,
area geometry NOT NULL
"#
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
        conn.execute("INSERT INTO station_overrides (nr_ref, area) VALUES ($1, $2)",
                     &[&nr_ref, &area])?;
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
    fn table_desc() -> &'static str {
        r#"
id SERIAL PRIMARY KEY,
nr_ref VARCHAR NOT NULL,
area geometry NOT NULL,
defect INT NOT NULL
"#
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
    fn indexes() -> Vec<&'static str> {
        vec![
            "links_p1 ON links (p1)",
            "links_p2 ON links (p2)",
            "links_geom ON links USING GIST (way)"
        ]
    }
    fn table_desc() -> &'static str {
        r#"
p1 BIGINT NOT NULL REFERENCES nodes ON DELETE CASCADE,
p2 BIGINT NOT NULL REFERENCES nodes ON DELETE CASCADE,
way geometry NOT NULL,
distance REAL NOT NULL,
UNIQUE(p1, p2)
"#
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
    fn table_desc() -> &'static str {
        r#"
s1 INT NOT NULL REFERENCES stations ON DELETE RESTRICT,
s2 INT NOT NULL REFERENCES stations ON DELETE RESTRICT,
way geometry NOT NULL,
nodes BIGINT[] NOT NULL,
crossings INT[] NOT NULL,
crossing_locations DOUBLE PRECISION[] NOT NULL,
id SERIAL PRIMARY KEY,
UNIQUE(s1, s2),
CHECK(cardinality(crossings) = cardinality(crossing_locations))
"#
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
    fn table_desc() -> &'static str {
        r#"
id SERIAL PRIMARY KEY,
name VARCHAR,
area geometry NOT NULL
"#
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
