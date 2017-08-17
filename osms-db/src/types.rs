use postgres::GenericConnection;
use postgres::rows::Row;
use postgres::types::ToSql;
use postgis::ewkb::{Point, LineString, Polygon};
use errors::*;

pub trait DbType: Sized {
    fn table_name() -> &'static str;
    fn table_desc() -> &'static str;
    fn from_row(row: &Row) -> Self;
    fn make_table<T: GenericConnection>(conn: &T) -> Result<()> {
        conn.execute(&format!("CREATE TABLE IF NOT EXISTS {} ({})",
                             Self::table_name(), Self::table_desc()), &[])?;
        Ok(())
    }
    fn from_select<T: GenericConnection>(conn: &T, where_clause: &str, args: &[&ToSql]) -> Result<Vec<Self>> {
        let query = format!("SELECT * FROM {} {}", Self::table_name(), where_clause);
        let qry = conn.query(&query, args)?;
        let mut ret = vec![];
        for row in &qry {
            ret.push(Self::from_row(&row));
        }
        Ok(ret)
    }

}
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
nr_ref VARCHAR UNIQUE NOT NULL,
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
p1 INT NOT NULL,
p2 INT NOT NULL,
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
    pub nodes: Vec<i32>
}
impl DbType for StationPath {
    fn table_name() -> &'static str {
        "station_paths"
    }
    fn table_desc() -> &'static str {
        r#"
s1 VARCHAR NOT NULL,
s2 VARCHAR NOT NULL,
way geometry NOT NULL,
nodes INT[] NOT NULL,
PRIMARY KEY(s1, s2)
"#
    }
    fn from_row(row: &Row) -> Self {
        Self {
            s1: row.get(0),
            s2: row.get(1),
            way: row.get(2),
            nodes: row.get(3)
        }
    }
}
impl StationPath {
    pub fn insert<T: GenericConnection>(&self, conn: &T) -> Result<()> {
        conn.execute("INSERT INTO station_paths (s1, s2, way, nodes) VALUES ($1, $2, $3, $4) ON CONFLICT DO UPDATE",
                     &[&self.s1, &self.s2, &self.way, &self.nodes])?;
        Ok(())
    }
}
#[derive(Debug, Clone)]
pub struct Train {
    pub id: i32,
    pub rsid: String,

}
