#[macro_use] extern crate nom;
extern crate chrono;

pub mod types;
#[cfg(test)]
mod tests;
pub use nom::IResult;
use types::*;
use nom::rest_s;
use chrono::*;
use std::error::Error;

named!(pub msn_header(&str) -> MsnHeader,
       map_res!(
           do_parse!(
               tag!("A") >>
               count!(tag!(" "), 29) >>
               tag!("FILE-SPEC=") >>
               ver: take!(8) >>
               date: take!(8) >>
               tag!(" ") >>
               time: take!(8) >>
               count!(tag!(" "), 3) >>
               seq: take!(2) >>
               rest_s >>
               (ver, date, time, seq)
               ),
               |(ver, date, time, s): (&str, &str, &str, &str)| -> Result<MsnHeader, Box<Error>> {
                   let seq = s.trim().parse::<u32>()?;
                   let time = NaiveTime::parse_from_str(time, "%H.%M.%S")?;
                   let date = NaiveDate::parse_from_str(date, "%d/%m/%y")?;
                   let dt = NaiveDateTime::new(date, time);
                   Ok(MsnHeader {
                       version: ver.into(),
                       timestamp: dt,
                       seq
                   })
               })
);

named!(pub cate_type(&str) -> CateType,
       alt_complete!(
           map!(tag!("0"), |_| CateType::Not) |
           map!(tag!("1"), |_| CateType::Small) |
           map!(tag!("2"), |_| CateType::Medium) |
           map!(tag!("3"), |_| CateType::Large) |
           map!(tag!("9"), |_| CateType::SubsidiaryTiploc)
           )
);

named!(pub msn_station(&str) -> MsnStation,
       map_res!(
           do_parse!(
               tag!("A") >>
               count!(tag!(" "), 4) >>
               name: take!(30) >>
               cate_type: cate_type >>
               tiploc: take!(7) >>
               subsidiary_crs: take!(3) >>
               count!(tag!(" "), 3) >>
               crs: take!(3) >>
               easting: take!(5) >>
               estimated: take!(1) >>
               northing: take!(5) >>
               change_time: take!(2) >>
               rest_s >>
               (name, cate_type, tiploc, subsidiary_crs, crs, easting, estimated,
                northing, change_time)
               ),
               |(name, cate_type, tiploc, subsidiary_crs, crs, easting, estimated, northing, change_time): (&str, CateType, &str, &str, &str, &str, &str, &str, &str)| -> Result<MsnStation, Box<Error>> {
                   let easting = easting.trim().parse::<u32>()?;
                   let northing = northing.trim().parse::<u32>()?;
                   let change_time = change_time.trim().parse::<u8>()?;
                   let estimated = estimated == "E";
                   Ok(MsnStation {
                       name: name.trim().into(),
                       cate_type,
                       tiploc: tiploc.trim().into(),
                       subsidiary_crs: subsidiary_crs.trim().into(),
                       crs: crs.trim().into(),
                       easting,
                       estimated,
                       northing,
                       change_time
                   })
               })
);
named!(pub msn_station_alias(&str) -> MsnStationAlias,
       map!(
           do_parse!(
               tag!("L") >>
               count!(tag!(" "), 4) >>
               name: take!(30) >>
               tag!(" ") >>
               alias: take!(30) >>
               count!(tag!(" "), 16) >>
               (name, alias)
               ),
               |(name, alias)| {
                   MsnStationAlias {
                       name: name.trim().into(),
                       alias: alias.trim().into()
                   }
               })
);
named!(pub msn_record(&str) -> MsnRecord,
       alt_complete!(
           map!(msn_header, |h| MsnRecord::Header(h)) |
           map!(msn_station, |h| MsnRecord::Station(h)) |
           map!(msn_station_alias, |h| MsnRecord::Alias(h)) |
           map!(rest_s, |h| MsnRecord::Other(h.into()))
           )
);

