//! Schedule storage component, storing and updating copies of CIF/ITPS schedules.

pub mod errors;
pub mod types;

use tspl_sqlite::initialize_db;

fn main() {
    initialize_db("./fahrplan.sqlite", &types::MIGRATIONS).unwrap();
}
