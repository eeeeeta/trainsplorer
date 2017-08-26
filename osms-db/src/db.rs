pub use postgres::GenericConnection;
pub use postgres::rows::Row;
use postgres::types::ToSql;
use errors::*;
use ntrod_types;
use osm::types::*;
use ntrod::types::*;
use osm::make::*;
use osm::org::*;
use util::*;

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
pub trait InsertableDbType: DbType {
    type Id;
    fn insert_self<T: GenericConnection>(&self, conn: &T) -> Result<Self::Id>;
}
pub fn initialize_database<T: GenericConnection>(conn: &T) -> Result<()> {
    debug!("initialize_database: making types...");
    conn.execute(ntrod_types::schedule::Days::create_type(), &[])?;
    conn.execute(ntrod_types::cif::StpIndicator::create_type(), &[])?;
    conn.execute(ScheduleLocation::create_type(), &[])?;
    debug!("initialize_database: making tables...");
    Node::make_table(conn)?;
    Link::make_table(conn)?;
    Station::make_table(conn)?;
    StationPath::make_table(conn)?;
    Schedule::make_table(conn)?;
    Train::make_table(conn)?;
    ScheduleWay::make_table(conn)?;
    Crossing::make_table(conn)?;
    ntrod_types::reference::CorpusEntry::make_table(conn)?;
    let mut changed = false;
    let mut nodes = count(conn, "FROM nodes", &[])?;
    if nodes == 0 {
        make_nodes(conn)?;
        nodes = count(conn, "FROM nodes", &[])?;
        changed = true;
        debug!("initialize_database: {} nodes after node creation", nodes);
    }
    let mut links = count(conn, "FROM links", &[])?;
    if links == 0 {
        make_links(conn)?;
        links = count(conn, "FROM links", &[])?;
        changed = true;
        debug!("initialize_database: {} links after link creation", links);
    }
    let mut stations = count(conn, "FROM stations", &[])?;
    if stations == 0 {
        make_stations(conn)?;
        stations = count(conn, "FROM stations", &[])?;
        changed = true;
        debug!("initialize_database: {} stations after station creation", stations);
    }
    let mut crossings = count(conn, "FROM crossings", &[])?;
    if crossings == 0 {
        make_crossings(conn)?;
        crossings = count(conn, "FROM crossings", &[])?;
        changed = true;
        debug!("initialize_database: {} crossings after crossing creation", crossings);
    }
    if changed {
        debug!("initialize_database: changes occurred, running connectifier...");
        the_great_connectifier(conn)?;
        debug!("initialize_database: {} nodes, {} links after connectification", nodes, links);
    }
    let unclassified = count(conn, "FROM nodes WHERE graph_part = 0", &[])?;
    if unclassified != 0 || changed {
        debug!("initialize_database: running node separator...");
        separate_nodes(conn)?;
    }
    debug!("initialize_database: database OK (nodes = {}, links = {}, stations = {}, crossings = {})",
           nodes, links, stations, crossings);
    Ok(())
}
