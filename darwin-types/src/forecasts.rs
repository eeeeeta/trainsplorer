//! Forecast data - http://www.thalesgroup.com/rtti/PushPort/Forecasts/v2
use chrono::{NaiveTime, NaiveDate};
use common::{CircularTimes, DisruptionReason};
use std::default::Default;
use std::str::FromStr;
use errors::*;
use deser::*;
use util;
use std::io::Read;
use xml::reader::{XmlEvent, EventReader};

#[derive(Debug, Clone)]
pub enum PlatformSource {
    Planned,
    Automatic,
    Manual
}
impl FromStr for PlatformSource {
    type Err = DarwinError;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "P" => Ok(PlatformSource::Planned),
            "A" => Ok(PlatformSource::Automatic),
            "M" => Ok(PlatformSource::Manual),
            x => Err(DarwinError::Expected("one of P, A, or M", x.into()))
        }
    }
}
#[derive(Default, Debug, Clone)]
/// Platform number with associated flags. 
pub struct PlatformData {
    pub platform: String,
    /// Platform number is suppressed and should not be displayed.
    pub platsup: bool,
    /// Whether a CIS, or Darwin Workstation, has set platform suppression
    /// at this location.
    pub cis_platsup: bool,
    /// The source of the platform number.
    pub platsrc: Option<PlatformSource>,
    /// Whether the platform number is confirmed.
    pub conf: bool
}
impl XmlDeserialize for PlatformData {
    fn from_xml_iter<R: Read>(se: XmlStartElement, reader: &mut EventReader<R>) -> Result<Self> {
        let mut ret: Self = Default::default();
        xml_attrs! { se, value,
            parse platsup, conf on ret,
            with platsrc on ret {
                Some(value.parse()?)
            },
            pat "cisPlatsup" => {
                ret.cis_platsup = value.parse()?;
            }
        }
        let mut plat = false;
        xml_iter! { se, reader,
            pat XmlEvent::Characters(data) => {
                ret.platform = data;
                plat = true;
            }
        }
        if !plat {
            Err(DarwinError::Missing("platform data"))?;
        }
        Ok(ret)
    }
}
#[derive(Default, Debug, Clone)]
/// Type describing time-based forecast attributes for a TS arrival/departure/pass.
pub struct TsTimeData {
    /// Estimated Time. For locations with a public activity, this will be based on the "public schedule". For all other activities, it will be based on the "working schedule".
    pub et: Option<NaiveTime>,
    /// The estimated time based on the "working schedule". This will only be set for public activities and when it also differs from the estimated time based on the "public schedule".
    pub wet: Option<NaiveTime>,
    /// Actual Time
    pub at: Option<NaiveTime>,
    /// If true, indicates that an actual time ("at") value has just been removed and replaced by an estimated time ("et"). Note that this attribute will only be set to "true" once, when the actual time is removed, and will not be set in any snapshot.
    pub at_removed: bool,
    /// The manually applied lower limit that has been applied to the estimated time at this location. The estimated time will not be set lower than this value, but may be set higher.
    pub etmin: Option<NaiveTime>,
    /// Indicates that an unknown delay forecast has been set for the estimated time at this location. Note that this value indicates where a manual unknown delay forecast has been set, whereas it is the "delayed" attribute that indicates that the actual forecast is "unknown delay".
    pub et_unknown: bool,
    /// Indicates that this estimated time is a forecast of "unknown delay". Displayed  as "Delayed" in LDB. Note that this value indicates that this forecast is "unknown delay", whereas it is the "etUnknown" attribute that indicates where the manual unknown delay forecast has been set.
    pub delayed: bool,
    /// The source of the forecast or actual time.
    pub src: Option<String>,
    /// The RTTI CIS code of the CIS instance if the src is a CIS.
    pub src_inst: Option<String>
}
impl XmlDeserialize for TsTimeData {
    fn from_xml_iter<R: Read>(se: XmlStartElement, reader: &mut EventReader<R>) -> Result<Self> {
        let mut ret: Self = Default::default();
        xml_attrs! { se, value,
            with et, wet, at, etmin on ret {
                Some(util::parse_time(&value)?)
            },
            with delayed on ret {
                value.parse()?
            },
            with src on ret {
                Some(value)
            },
            pat "srcInst" => {
                ret.src_inst = Some(value);
            },
            pat "atRemoved" => {
                ret.at_removed = value.parse()?;
            },
            pat "etUnknown" => {
                ret.et_unknown = value.parse()?;
            }
        }
        xml_iter! { se, reader, }
        Ok(ret)
    }
}
#[derive(Default, Debug, Clone)]
/// Forecast data for an individual location in the service's schedule.
pub struct TsLocation {
    /// TIPLOC
    pub tiploc: String,
    /// Scheduled timing data for this location.
    pub timings: CircularTimes,
    /// Forecast data for the arrival at this location.
    pub arr: Option<TsTimeData>,
    /// Forecast data for the departure at this location.
    pub dep: Option<TsTimeData>,
    /// Forecast data for the pass of this location.
    pub pass: Option<TsTimeData>,
    /// Current platform number.
    pub plat: Option<PlatformData>,
    /// Whether the service is suppressed at this location or not.
    pub suppr: bool,
    /// The length of the service at this location on departure (or arrival at destination). The default value of zero indicates that the length is unknown.
    pub length: u32,
    /// Indicates from which end of the train stock will be detached. The value is set to “true” if stock will be detached from the front of the train at this location. It will be set at each location where stock will be detached from the front. Darwin will not validate that a stock detachment activity code applies at this location.
    pub detach_front: bool
}
impl XmlDeserialize for TsLocation {
    fn from_xml_iter<R: Read>(se: XmlStartElement, reader: &mut EventReader<R>) -> Result<Self> {
        let mut ret: Self = Default::default();
        let mut tpl = false;
        {
            let timings = &mut ret.timings;
            xml_attrs! { se, value,
                with wta, wtd, wtp, pta, ptd on timings {
                    Some(util::parse_time(&value)?)
                },
                pat "tpl" => {
                    tpl = true;
                    ret.tiploc = value;
                }
            }
        }
        if !tpl {
            Err(DarwinError::Missing("TIPLOC"))?;
        }
        type BoolElement = util::ValueElement<bool>;
        type U32Element = util::ValueElement<u32>;
        xml_iter! { se, reader, 
            parse "arr", TsTimeData as arr {
                ret.arr = Some(arr);
            },
            parse "dep", TsTimeData as dep {
                ret.dep = Some(dep);
            },
            parse "pass", TsTimeData as pass {
                ret.pass = Some(pass);
            },
            parse "suppr", BoolElement as suppr {
                ret.suppr = suppr.0;
            },
            parse "detachFront", BoolElement as detach_front {
                ret.detach_front = detach_front.0
            },
            parse "length", U32Element as len {
                ret.length = len.0;
            },
            parse "plat", PlatformData as plat {
                ret.plat = Some(plat);
            },
        }
        Ok(ret)
    }
}
/// Train Status. Update to the "real time" forecast data for a service.
#[derive(Debug, Clone)]
pub struct Ts {
    /// Late running reason for this service. The reason applies to all locations of this service.
    pub late_reason: Option<DisruptionReason>,
    /// Update of forecast data for individual locations in the service's schedule.
    pub locations: Vec<TsLocation>,
    /// RTTI unique Train Identifier.
    pub rid: String,
    /// Train UID.
    pub uid: String,
    /// Scheduled Start Date. *[editor's note: this is the date the train runs on, NOT the
    /// ITPS schedule's start date.]*
    pub start_date: NaiveDate,
    /// Indicates whether a train that divides is working with portions in reverse to their normal formation. The value applies to the whole train. Darwin will not validate that a divide association actually exists for this service.
    pub is_reverse_formation: bool
}
impl XmlDeserialize for Ts {
    fn from_xml_iter<R: Read>(se: XmlStartElement, reader: &mut EventReader<R>) -> Result<Self> {
        let mut rid = None;
        let mut uid = None;
        let mut ssd = None;
        let mut is_reverse_formation = false;
        xml_attrs! { se, value,
            pat "isReverseFormation" => {
                is_reverse_formation = value.parse()?;
            },
            pat "ssd" => {
                ssd = Some(NaiveDate::parse_from_str(&value, "%Y-%m-%d")?);
            },
            pat "rid" => {
                rid = Some(value);
            },
            pat "uid" => {
                uid = Some(value);
            }
        }
        let mut locs = vec![];
        let mut lr = None;
        xml_iter! { se, reader,
            parse "Location", TsLocation as loc {
                locs.push(loc);
            },
            parse "LateReason", DisruptionReason as l {
                lr = Some(l);
            },
        }
        Ok(Ts {
            rid: rid.ok_or(DarwinError::Missing("rid"))?,
            uid: uid.ok_or(DarwinError::Missing("uid"))?,
            start_date: ssd.ok_or(DarwinError::Missing("ssd"))?,
            is_reverse_formation,
            locations: locs,
            late_reason: lr
        })
    }
}
