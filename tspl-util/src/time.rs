//! Time calculations for movement queries.

use chrono::prelude::*;
use chrono::Duration;

/// Given a timestamp and a duration, calculates a starting and ending time to search between on
/// that day, ignoring the fact that time wraps around across midnight.
///
/// Hey, it does what it says on the tin, right?
pub fn calculate_non_midnight_aware_times(ts: NaiveDateTime, within_dur: Duration) -> (NaiveTime, NaiveTime) {
    let start_ts = ts - within_dur;
    let start_time = if start_ts.date() != ts.date() {
        // Wraparound occurred, just return the start of the day (i.e. saturate)
        NaiveTime::from_hms(0, 0, 0)
    }
    else {
        start_ts.time()
    };
    let end_ts = ts + within_dur;
    let end_time = if end_ts.date() != ts.date() {
        // Same as above, but the other way this time.
        NaiveTime::from_hms(23, 59, 59)
    }
    else {
        end_ts.time()
    };
    (start_time, end_time)
}
