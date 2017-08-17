use chrono::*;
use serde::*;

pub fn str_to_time(x: &str) -> Option<NaiveTime> {
    let half = x.contains("H");
    let x = x.replace("H", "");
    let time = NaiveTime::parse_from_str(&x, "%H:%M");
    if let Ok(t) = time {
        if half {
            t.with_second(30)
        }
        else {
            Some(t)
        }
    }
    else {
        None
    }
}
pub fn parse_24h_time<'de, D>(d: D) -> Result<Option<NaiveTime>, D::Error> where D: Deserializer<'de> {
    Deserialize::deserialize(d)
        .map(|x: Option<String>| {
            if let Some(x) = x {
                str_to_time(&x)
            }
            else {
                None
            }
        })
}
pub fn parse_24h_time_force<'de, D>(d: D) -> Result<NaiveTime, D::Error> where D: Deserializer<'de> {
    Deserialize::deserialize(d)
        .map(|x: String| {
            str_to_time(&x)
                .unwrap_or(NaiveTime::from_hms(0, 0, 0))
        })
}
