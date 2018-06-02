pub mod types;

use db::*;
use ntrod_types::reference::CorpusEntry;

pub fn get_stanox_for_tiploc<T: GenericConnection>(conn: &T, tiploc: &str) -> Result<Option<String>, ::failure::Error> {
    let entries = CorpusEntry::from_select(conn, "WHERE tiploc = $1 AND stanox IS NOT NULL", &[&tiploc])?;
    for entry in entries {
        return Ok(Some(entry.stanox.unwrap()));
    }
    Ok(None)
}
