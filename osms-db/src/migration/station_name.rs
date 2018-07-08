use errors::*;
use osm;
use postgres::transaction::Transaction;

pub fn station_name_func(conn: &Transaction) -> Result<()> {
    debug!("updating station names to comply with new schema");
    for row in &conn.query("SELECT (id, nr_ref) FROM stations", &[])? {
        let id: i32 = row.get(0);
        let tiploc: String = row.get(1);
        trace!("processing station {}", tiploc);
        let name = osm::tiploc_to_readable(conn, &tiploc)?;
        conn.execute("UPDATE stations SET name = $1 WHERE id = $2", &[&name, &id])?;
    }
    Ok(())
}
