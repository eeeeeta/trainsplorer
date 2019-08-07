//! Handling train activations, and shunting schedule data about the place.

use crate::errors::*;
use crate::types::*;
use tspl_sqlite::traits::Connection;

/// Activate a train from the specified schedule details. 
pub fn process_activation(conn: &Connection, uid: &str, stp_indicator: &str, start_date: NaiveDate, source: u8) -> Result<i64> {
    unimplemented!()
}
