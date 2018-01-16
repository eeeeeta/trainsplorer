use chrono::*;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct MsnHeader {
    pub version: String,
    pub timestamp: NaiveDateTime,
    pub seq: u32
}
#[repr(u8)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum CateType {
    Not = 0,
    Small = 1,
    Medium = 2,
    Large = 3,
    SubsidiaryTiploc = 9
}
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct MsnStation {
    pub name: String,
    pub cate_type: CateType,
    pub tiploc: String,
    pub subsidiary_crs: String,
    pub crs: String,
    pub easting: u32,
    pub estimated: bool,
    pub northing: u32,
    pub change_time: u8
}
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct MsnStationAlias {
    pub name: String,
    pub alias: String
}
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum MsnRecord {
    Header(MsnHeader),
    Station(MsnStation),
    Alias(MsnStationAlias),
    Other(String)
}

