use types::*;
use serde_json;

#[test]
fn parse_schedule_v1() {
    let data = include_str!("schedule_v1.json");
    let _: Record = serde_json::from_str(&data).unwrap();
}
