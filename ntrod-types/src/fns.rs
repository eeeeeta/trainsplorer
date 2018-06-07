use chrono::*;
use chrono_tz::Europe::London;
use chrono_tz::Tz;
use serde::*;
use std::fmt::Display;
use std::str::FromStr;
use super::schedule::Days;
use super::cif::StpIndicator;

pub fn parse_vstp_time<'de, D>(d: D) -> Result<Option<NaiveTime>, D::Error> where D: Deserializer<'de> {
    Deserialize::deserialize(d)
        .map(|x: Option<String>| {
            if let Some(x) = x {
                if let Ok(x) = NaiveTime::parse_from_str(x.trim(), "%H%M%S") {
                    return Some(x);
                }
            }
            None
        })
}
pub fn parse_vstp_time_force<'de, D>(d: D) -> Result<NaiveTime, D::Error> where D: Deserializer<'de> {
    let x: String = Deserialize::deserialize(d)?;
    match NaiveTime::parse_from_str(x.trim(), "%H%M%S") {
        Ok(res) => Ok(res),
        Err(e) => Err(de::Error::custom(format!("failed to parse a VSTP time {}: {}", x, e)))
    }
}
pub fn str_to_time(x: &str) -> Option<NaiveTime> {
    let half = x.contains("H");
    let x = x.replace("H", "");
    let time = NaiveTime::parse_from_str(&x, "%H%M");
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
                str_to_time(x.trim())
            }
            else {
                None
            }
        })
}
pub fn parse_24h_time_force<'de, D>(d: D) -> Result<NaiveTime, D::Error> where D: Deserializer<'de> {
    let x: String = Deserialize::deserialize(d)?;
    match str_to_time(x.trim()) {
        Some(res) => Ok(res),
        None => Err(de::Error::custom(format!("failed to parse a 24h time: {}", x)))
    }
}
pub fn naive_date_to_london<'de, D>(d: D) -> Result<Date<Tz>, D::Error> where D: Deserializer<'de> {
    let d: NaiveDate = Deserialize::deserialize(d)?;
    Ok(London.from_local_date(&d).single().ok_or("Timezone fail").map_err(de::Error::custom)?)
}
pub fn fix_buggy_schedule_type<'de, D>(d: D) -> Result<StpIndicator, D::Error> where D: Deserializer<'de> {
    Deserialize::deserialize(d)
        .map(|x: StpIndicator| {
            match x {
                StpIndicator::Overlay => StpIndicator::Permanent,
                StpIndicator::Permanent => StpIndicator::Overlay,
                x => x
            }
        })
}
pub fn parse_days<'de, D>(d: D) -> Result<Days, D::Error> where D: Deserializer<'de> {
    Deserialize::deserialize(d)
        .map(|x: String| {
            let mut chars = x.chars();
            Days {
                mon: chars.next().map(|x| x == '1').unwrap_or(false),
                tue: chars.next().map(|x| x == '1').unwrap_or(false),
                wed: chars.next().map(|x| x == '1').unwrap_or(false),
                thu: chars.next().map(|x| x == '1').unwrap_or(false),
                fri: chars.next().map(|x| x == '1').unwrap_or(false),
                sat: chars.next().map(|x| x == '1').unwrap_or(false),
                sun: chars.next().map(|x| x == '1').unwrap_or(false),
            }
        })

}
pub fn from_str<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where T: FromStr,
          T::Err: Display,
          D: Deserializer<'de>
{
    let s = String::deserialize(deserializer)?;
    T::from_str(&s).map_err(de::Error::custom)
}
pub fn non_empty_str<'de, D>(deserializer: D) -> Result<String, D::Error>
    where D: Deserializer<'de>
{
    let s = String::deserialize(deserializer)?;
    if s.trim() == "" {
        Err(de::Error::custom("expected non-empty string, got empty string"))
    }
    else {
        Ok(s)
    }
}
pub fn non_empty_str_opt<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
    where D: Deserializer<'de>
{
    let s: Option<String> = Deserialize::deserialize(deserializer)?;
    Ok(if let Some(s) = s {
        if s.trim() == "" { None }
        else { Some(s) }
    }
    else {
        None
    })
}


pub fn from_str_opt<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
    where T: FromStr,
          T::Err: Display,
          D: Deserializer<'de>
{
    let s: Option<String> = Deserialize::deserialize(deserializer)?;
    Ok(if let Some(s) = s {
        if s == "" {
            None
        }
        else {
            Some(T::from_str(&s).map_err(de::Error::custom)?)
        }
    }
    else {
        None
    })
}
pub fn from_str_trimming<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where T: FromStr,
          T::Err: Display,
          D: Deserializer<'de>
{
    let s = String::deserialize(deserializer)?;
    T::from_str(s.trim()).map_err(de::Error::custom)
}
pub fn from_str_opt_trimming<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
    where T: FromStr,
          T::Err: Display,
          D: Deserializer<'de>
{
    let s: Option<String> = Deserialize::deserialize(deserializer)?;
    Ok(if let Some(s) = s {
        if s == "" {
            None
        }
        else {
            Some(T::from_str(s.trim()).map_err(de::Error::custom)?)
        }
    }
       else {
           None
       })
}


pub fn parse_ts_opt<'de, D>(d: D) -> Result<Option<NaiveDateTime>, D::Error> where D: Deserializer<'de> {
    let s: Option<u64> = from_str_opt(d)?;
    Ok(if let Some(ms) = s {
        let secs = ms / 1000;
        let nanos = (ms - (secs * 1000)) * 1000000;
        Some(NaiveDateTime::from_timestamp(secs as _, nanos as _))
    } else {
        None
    })
}
pub fn parse_ts<'de, D>(d: D) -> Result<NaiveDateTime, D::Error> where D: Deserializer<'de> {
    Ok(parse_ts_opt(d)?.ok_or(de::Error::custom("expected timestamp, got nothing"))?)
}
