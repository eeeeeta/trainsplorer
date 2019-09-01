//! Providing station suggestions (i.e. autocomplete in the station box).

use tspl_sqlite::TsplPool;
use tspl_sqlite::traits::*;
use tspl_nennen::types::*;
use serde_derive::Serialize;

use crate::errors::*;

#[derive(Serialize)]
pub struct StationSuggestion {
    name: String,
    code: String,
    code_type: String
}
#[derive(Serialize)]
pub struct StationSuggestions {
    suggestions: Vec<StationSuggestion>
}

pub struct StationSuggester {
    pool: TsplPool
}

impl StationSuggester {
    pub fn new(pool: TsplPool) -> Self {
        Self { pool }
    }
    pub fn suggestions_for(&self, query: &str) -> WebResult<StationSuggestions> {
        let db = self.pool.get()?;
        let names = IndexedStationName::from_select(&db, "WHERE name MATCH ?1 OR crs MATCH ?1 OR tiploc MATCH ?1 ORDER BY rank LIMIT 6", params![query])?;
        let ret = names.into_iter()
            .filter_map(|x| {
                if x.tiploc.is_some() {
                    Some(StationSuggestion {
                        name: x.name,
                        code: x.tiploc.unwrap(),
                        code_type: "TIPLOC".into()
                    })
                }
                else {
                    None
                    // FIXME: CRS codes aren't supported by the
                    // current basic movement search code.
                    /*
                    StationSuggestion {
                        name: x.name,
                        code: x.crs.unwrap(),
                        code_type: "CRS".into()
                    }
                    */
                }
            })
            .collect();
        Ok(StationSuggestions { suggestions: ret })
    }
}
