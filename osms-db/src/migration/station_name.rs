use db::*;
use errors::*;
use osm::types::*;
use osm;
use postgres::transaction::Transaction;

pub fn station_name_func(conn: &Transaction) -> Result<()> {
    debug!("updating station names to comply with new schema");
    for sta in Station::from_select(conn, "", &[])? {
        trace!("processing station {}", sta.nr_ref);
        let name = osm::tiploc_to_readable(conn, &sta.nr_ref)?;
        conn.execute("UPDATE stations SET name = $1 WHERE id = $2", &[&name, &sta.id])?;
    }
    Ok(())
}
