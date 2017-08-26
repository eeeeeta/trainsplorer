use postgis::ewkb::{Point, LineString, Polygon};
use db::{DbType, InsertableDbType, GenericConnection, Row};
use errors::*;
use util::*;
use geo;

#[derive(Debug, Clone)]
pub struct Node {
    pub id: i32,
    pub location: Point,
    pub distance: f32,
    pub parent: Option<i32>,
    pub visited: bool,
    pub graph_part: i32,
    pub parent_geom: Option<LineString>
}
impl DbType for Node {
    fn table_name() -> &'static str {
        "nodes"
    }
    fn table_desc() -> &'static str {
        r#"
id SERIAL PRIMARY KEY,
location geometry NOT NULL,
distance REAL NOT NULL DEFAULT 'Infinity',
parent INT,
visited BOOL NOT NULL DEFAULT false,
graph_part INT NOT NULL DEFAULT 0,
parent_geom geometry
"#
    }
    fn from_row(row: &Row) -> Self {
        Self {
            id: row.get(0),
            location: row.get(1),
            distance: row.get(2),
            parent: row.get(3),
            visited: row.get(4),
            graph_part: row.get(5),
            parent_geom: row.get(6)
        }
    }
}
impl Node {
    pub fn insert<T: GenericConnection>(conn: &T, location: Point) -> Result<i32> {
        for row in &conn.query("SELECT id FROM nodes WHERE location = $1",
                               &[&location])? {
            return Ok(row.get(0));
        }
        let qry = conn.query("INSERT INTO nodes (location) VALUES ($1) RETURNING id",
                             &[&location])?;
        let mut ret = None;
        for row in &qry {
            ret = Some(row.get(0))
        }
        Ok(ret.expect("Somehow, we never got an id in Node::insert..."))
    }
    pub fn new_at_point<T: GenericConnection>(conn: &T, point: Point) -> Result<(i32, bool)> {
        use geo::algorithm::distance::Distance;
        use geo::algorithm::split::Split;
        use geo::algorithm::haversine_length::HaversineLength;
        let trans = conn.transaction()?;
        let pt = geo::Point::from_postgis(&point);
        let mut okay = false;
        let prev_nodes = Node::from_select(&trans, "WHERE location = $1", &[&point])?;
        if prev_nodes.len() > 0 {
            okay = true;
        }
        let node = Self::insert(&trans, point.clone())?;
        let qry = Link::from_select(&trans, "", &[])?;
        let mut links = vec![];
        for link in qry {
            let line = geo::LineString::from_postgis(&link.way);
            if line.distance(&pt) <= 0.00000001 {
                if line.0.first().map(|x| x == &pt).unwrap_or(false) ||
                    line.0.last().map(|x| x == &pt).unwrap_or(false) {
                    okay = true;
                    continue;
                }
                links.push(link);
            }
        }
        debug!("making new at point {:?}: {} links", point, links.len());
        for link in links {
            okay = true;
            let line = geo::LineString::from_postgis(&link.way);
            let vec = line.split(&pt, 0.00000001);
            if vec.len() != 2 {
                bail!("expected 2 results after split, got {}", vec.len());
            }
            let mut iter = vec.into_iter();
            let (first, last) = (iter.next().unwrap(), iter.next().unwrap());
            let (df, dl) = (first.haversine_length(), last.haversine_length());
            let (first, last) = (geo_ls_to_postgis(first), geo_ls_to_postgis(last));
            trans.execute(
                "UPDATE links SET p2 = $1, way = $2, distance = $3 WHERE p1 = $4 AND p2 = $5",
                &[&node, &first, &(df as f32), &link.p1, &link.p2]
            )?;
            let new = Link {
                p1: node,
                p2: link.p2,
                distance: dl as f32,
                way: last
            };
            debug!("new setup: {} <-> {} <-> {}", link.p1, node, link.p2);
            new.insert(&trans)?;
        }
        trans.commit()?;
        Ok((node, okay))
    }
}
#[derive(Debug, Clone)]
pub struct Station {
    pub nr_ref: String,
    pub point: i32,
    pub area: Polygon
}
impl DbType for Station {
    fn table_name() -> &'static str {
        "stations"
    }
    fn table_desc() -> &'static str {
        r#"
nr_ref VARCHAR PRIMARY KEY,
point INT NOT NULL,
area geometry NOT NULL
"#
    }
    fn from_row(row: &Row) -> Self {
        Self {
            nr_ref: row.get(0),
            point: row.get(1),
            area: row.get(2),
        }
    }
}
impl Station {
    pub fn insert<T: GenericConnection>(conn: &T, nr_ref: &str, point: i32, area: Polygon) -> Result<()> {
        conn.execute("INSERT INTO stations (nr_ref, point, area) VALUES ($1, $2, $3)",
                     &[&nr_ref, &point, &area])?;
        Ok(())
    }

}
#[derive(Debug, Clone)]
pub struct Link {
    pub p1: i32,
    pub p2: i32,
    pub way: LineString,
    pub distance: f32
}
impl DbType for Link {
    fn table_name() -> &'static str {
        "links"
    }
    fn table_desc() -> &'static str {
        r#"
p1 INT NOT NULL REFERENCES nodes ON DELETE CASCADE,
p2 INT NOT NULL REFERENCES nodes ON DELETE CASCADE,
way geometry NOT NULL,
distance REAL NOT NULL
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
        conn.execute("INSERT INTO links (p1, p2, way, distance) VALUES ($1, $2, $3, $4)",
                     &[&self.p1, &self.p2, &self.way, &self.distance])?;
        Ok(())
    }
}
#[derive(Debug, Clone)]
pub struct StationPath {
    pub s1: String,
    pub s2: String,
    pub way: LineString,
    pub nodes: Vec<i32>,
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
s1 VARCHAR NOT NULL REFERENCES stations ON DELETE RESTRICT,
s2 VARCHAR NOT NULL REFERENCES stations ON DELETE RESTRICT,
way geometry NOT NULL,
nodes INT[] NOT NULL,
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
        for row in &conn.query("SELECT id FROM station_paths
                                WHERE s1 = $1 AND s2 = $2",
                               &[&self.s1, &self.s2])? {
            return Ok(row.get(0));
        }
        let qry = conn.query("INSERT INTO station_paths
                              (s1, s2, way, nodes, crossings, crossing_locations)
                              VALUES ($1, $2, $3, $4, $5, $6)
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
    pub node_id: i32,
    pub name: Option<String>,
    pub other_node_ids: Vec<i32>,
    pub area: Polygon
}
impl DbType for Crossing {
    fn table_name() -> &'static str {
        "crossings"
    }
    fn table_desc() -> &'static str {
        r#"
node_id INT PRIMARY KEY REFERENCES nodes ON DELETE RESTRICT,
name VARCHAR,
other_node_ids INT[] NOT NULL,
area geometry NOT NULL
"#
    }
    fn from_row(row: &Row) -> Self {
        Self {
            node_id: row.get(0),
            name: row.get(1),
            other_node_ids: row.get(2),
            area: row.get(3)
        }
    }
}
impl InsertableDbType for Crossing {
    type Id = i32;
    fn insert_self<T: GenericConnection>(&self, conn: &T) -> Result<i32> {
        let qry = conn.query("INSERT INTO crossings
                              (node_id, name, other_node_ids, area)
                              VALUES ($1, $2, $3, $4)
                              RETURNING node_id",
                             &[&self.node_id, &self.name,
                               &self.other_node_ids, &self.area])?;
        let mut ret = None;
        for row in &qry {
            ret = Some(row.get(0))
        }
        Ok(ret.expect("No id in Crossing::insert?!"))
    }
}
