//! Utility functions.

use chrono::prelude::*;

pub fn action_to_str(act: u8) -> &'static str {
    match act {
        0 => "arr",
        1 => "dep",
        2 => "pass",
        _ => "?"
    }
}
pub fn action_to_icon(act: u8) -> &'static str {
    match act {
        0 => "arrow-alt-circle-right",
        1 => "arrow-alt-circle-left",
        2 => "arrow-alt-circle-up",
        _ => "?"
    }
}
pub fn action_past_tense(act: u8) -> &'static str {
    match act {
        0 => "Arrived",
        1 => "Departed",
        2 => "Passed through",
        _ => "???"
    }
}
pub fn format_time_with_half(time: &NaiveTime) -> String {
    match time.second() {
        0 => time.format("%H:%M").to_string(),
        30 => time.format("%H:%M½").to_string(),
        _ => time.format("%H:%M:%S").to_string()
    }
}

