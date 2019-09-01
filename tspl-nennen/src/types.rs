//! Reference data database types.

use tspl_sqlite::traits::*;
use tspl_sqlite::migrations::Migration;
use tspl_sqlite::migration;

pub use ntrod_types::reference::CorpusEntry;
pub use atoc_msn::types::MsnStation;
use atoc_msn::types::CateType;

pub static MIGRATIONS: [Migration; 1] = [
    migration!(0, "initial")
];

/// A mapping of station names to TIPLOC and/or CRS codes.
pub struct StationName {
    /// Internal ID.
    pub id: i64,
    /// The station's human-readable name.
    ///
    /// e.g. 'Clapham Junction', 'Waterloo East'
    pub name: String,
    /// The station's Timing Point Location (TIPLOC) code.
    pub tiploc: Option<String>,
    /// The station's Customer Reservation System (CRS) code.
    pub crs: Option<String>
}
impl DbType for StationName {
    fn table_name() -> &'static str {
        "station_names"
    }
    fn from_row(row: &Row, s: usize) -> RowResult<Self> {
        Ok(Self {
            id: row.get(s + 0)?,
            name: row.get(s + 1)?,
            tiploc: row.get(s + 2)?,
            crs: row.get(s + 3)?,
        })
    }
}
impl InsertableDbType for StationName {
    type Id = ();
    fn insert_self(&self, conn: &Connection) -> RowResult<()> {
        let mut stmt = conn.prepare("INSERT INTO station_names
                        (name, tiploc, crs)
                        VALUES (?, ?, ?)
                        ON CONFLICT DO NOTHING")?;
        stmt.execute(params![self.name, self.tiploc, self.crs])?;
        Ok(())
    }
}

/// Wrapper for the CorpusEntry type, in order to not violate
/// the orphan rules.
pub struct WrappedCorpusEntry(pub CorpusEntry);

impl DbType for WrappedCorpusEntry {
    fn table_name() -> &'static str {
        "corpus_entries"
    }
    fn from_row(row: &Row, s: usize) -> RowResult<Self> {
        Ok(Self(CorpusEntry {
            stanox: row.get(s + 0)?,
            uic: row.get(s + 1)?,
            crs: row.get(s + 2)?,
            tiploc: row.get(s + 3)?,
            nlc: row.get(s + 4)?,
            nlcdesc: row.get(s + 5)?,
            nlcdesc16: row.get(s + 6)?,
        }))
    }
}
impl InsertableDbType for WrappedCorpusEntry {
    type Id = ();
    fn insert_self(&self, conn: &Connection) -> RowResult<()> {
        let mut stmt = conn.prepare("INSERT INTO corpus_entries
                        (stanox, uic, crs, tiploc, nlc, nlcdesc, nlcdesc16)
                        VALUES (?, ?, ?, ?, ?, ?, ?)
                        ON CONFLICT DO NOTHING")?;
        stmt.insert(params![self.0.stanox, self.0.uic, self.0.crs, self.0.tiploc, self.0.nlc,
                            self.0.nlcdesc, self.0.nlcdesc16])?;
        Ok(())
    }
}

/// Wrapper for the MsnStation type, in order to not violate
/// the orphan rules.
pub struct WrappedMsnStation(pub MsnStation);

impl DbType for WrappedMsnStation {
    fn table_name() -> &'static str {
        "msn_entries"
    }
    fn from_row(row: &Row, s: usize) -> RowResult<Self> {
        Ok(Self(MsnStation {
            name: row.get(s + 0)?,
            // FIXME: this is hardcoded, because doing
            // the conversion is nontrivial, and we don't
            // even use this field, so I'm not going to bother...
            cate_type: CateType::Not,
            tiploc: row.get(s + 2)?,
            subsidiary_crs: row.get(s + 3)?,
            crs: row.get(s + 4)?,
            easting: row.get(s + 5)?,
            estimated: row.get(s + 6)?,
            northing: row.get(s + 7)?,
            change_time: row.get(s + 8)?,
        }))
    }
}
impl InsertableDbType for WrappedMsnStation {
    type Id = ();
    fn insert_self(&self, conn: &Connection) -> RowResult<()> {
        let mut stmt = conn.prepare("INSERT INTO msn_entries
                        (name, cate_type, tiploc, subsidiary_crs, crs, easting, estimated,
                         northing, change_time)
                        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                        ON CONFLICT DO NOTHING")?;
        stmt.insert(params![self.0.name, self.0.cate_type as u8, self.0.tiploc,
                            self.0.subsidiary_crs, self.0.crs, self.0.easting,
                            self.0.estimated, self.0.northing, self.0.change_time])?;
        Ok(())
    }
}
