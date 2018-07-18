//! Schedule data - http://www.thalesgroup.com/rtti/PushPort/Schedules/v1
use chrono::{NaiveTime, NaiveDate};
use common::DisruptionReason;
use std::io::Read;
use deser::*;
use errors::*;
use xml::reader::EventReader;

/// Group of attributes common to all schedule locations.
#[derive(Builder, Debug, Clone)]
#[builder(private)]
pub struct SchedLocAttributes {
    /// TIPLOC.
    pub tpl: String,
    /// Current activity codes.
    #[builder(default = "\"  \".into()")]
    pub act: String,
    /// Planned activity codes (if different to current activities).
    #[builder(default)]
    pub plan_act: Option<String>,
    /// Whether or not the train is cancelled at this location.
    #[builder(default)]
    pub can: bool
}
/// Group of attributes common to all calling points.
#[derive(Builder, Debug, Clone)]
#[builder(private)]
pub struct CallPtAttributes {
    /// Public scheduled time of arrival.
    #[builder(default)]
    pub pta: Option<NaiveTime>,
    /// Public scheduled time of departure.
    #[builder(default)]
    pub ptd: Option<NaiveTime>
}
/// A location in a train schedule.
///
/// See the documentation on each of the structs in this enum for more.
#[derive(Debug, Clone)]
pub enum ScheduleLocation {
    Or(LocOr),
    OpOr(LocOpOr),
    Ip(LocIp),
    OpIp(LocOpIp),
    Pp(LocPp),
    Dt(LocDt),
    OpDt(LocOpDt)
}
/// Passenger origin calling point ('OR') element.
#[derive(Builder, Debug, Clone)]
#[builder(private)]
pub struct LocOr {
    /// Common attributes for this schedule location.
    pub sla: SchedLocAttributes,
    /// Common attributes for this calling point.
    pub cpa: CallPtAttributes,
    /// Working scheduled time of arrival.
    #[builder(default)]
    pub wta: Option<NaiveTime>,
    /// Working scheduled time of departure.
    pub wtd: NaiveTime,
    /// TIPLOC of a False Destination to be used at this location.
    #[builder(default)]
    pub fd: Option<String>,
}
schedule_loc_deserialize! { LocOr, LocOrBuilder, ret, value,
    sla sla,
    cpa cpa,
    time wtd on ret,
    time_opt wta on ret,
    with fd on ret {
        Some(value)
    },
}
/// Operational origin ('OPOR') element.
#[derive(Builder, Debug, Clone)]
#[builder(private)]
pub struct LocOpOr {
    /// Common attributes for this schedule location.
    pub sla: SchedLocAttributes,
    /// Working scheduled time of arrival.
    #[builder(default)]
    pub wta: Option<NaiveTime>,
    /// Working scheduled time of departure.
    pub wtd: NaiveTime,
}
schedule_loc_deserialize! { LocOpOr, LocOpOrBuilder, ret, value,
    sla sla,
    time wtd on ret,
    time_opt wta on ret,
}
/// Passenger intermediate calling point ('IP') element.
#[derive(Builder, Debug, Clone)]
#[builder(private)]
pub struct LocIp {
    /// Common attributes for this schedule location.
    pub sla: SchedLocAttributes,
    /// Common attributes for this calling point.
    pub cpa: CallPtAttributes,
    /// Working scheduled time of arrival.
    pub wta: NaiveTime,
    /// Working scheduled time of departure.
    pub wtd: NaiveTime,
    /// Delay value (in minutes) implied by a change to the service's route.
    ///
    /// This value has been added to the forecast lateness of the service at
    /// the previous schedule location when calculating the expected lateness
    /// of arrival at this location.
    #[builder(default)]
    pub rdelay_mins: i32,
    /// TIPLOC of a False Destination to be used at this location.
    #[builder(default)]
    pub fd: Option<String>,
}
schedule_loc_deserialize! { LocIp, LocIpBuilder, ret, value,
    sla sla,
    cpa cpa,
    time wtd, wta on ret,
    parse rdelay_mins from rdelay on ret,
    with fd on ret {
        Some(value)
    },
}
/// Operational intermediate point ('OPIP') element.
#[derive(Builder, Debug, Clone)]
#[builder(private)]
pub struct LocOpIp {
    /// Common attributes for this schedule location.
    pub sla: SchedLocAttributes,
    /// Working scheduled time of arrival.
    pub wta: NaiveTime,
    /// Working scheduled time of departure.
    pub wtd: NaiveTime,
    /// Delay value (in minutes) implied by a change to the service's route.
    ///
    /// This value has been added to the forecast lateness of the service at
    /// the previous schedule location when calculating the expected lateness
    /// of arrival at this location.
    #[builder(default)]
    pub rdelay_mins: i32,
}
schedule_loc_deserialize! { LocOpIp, LocOpIpBuilder, ret, value,
    sla sla,
    time wtd, wta on ret,
    parse rdelay_mins from rdelay on ret,
}
/// Intermediate passing point ('PP') element.
#[derive(Builder, Debug, Clone)]
#[builder(private)]
pub struct LocPp {
    /// Common attributes for this schedule location.
    pub sla: SchedLocAttributes,
    /// Working scheduled time of passing.
    pub wtp: NaiveTime,
    /// Delay value (in minutes) implied by a change to the service's route.
    ///
    /// This value has been added to the forecast lateness of the service at
    /// the previous schedule location when calculating the expected lateness
    /// of arrival at this location.
    #[builder(default)]
    pub rdelay_mins: i32,
}
schedule_loc_deserialize! { LocPp, LocPpBuilder, ret, value,
    sla sla,
    time wtp on ret,
    parse rdelay_mins from rdelay on ret,
}
/// Passenger destination calling point ('DT') element.
#[derive(Builder, Debug, Clone)]
#[builder(private)]
pub struct LocDt {
    /// Common attributes for this schedule location.
    pub sla: SchedLocAttributes,
    /// Common attributes for this calling point.
    pub cpa: CallPtAttributes,
    /// Working scheduled time of arrival.
    pub wta: NaiveTime,
    /// Working scheduled time of departure.
    #[builder(default)]
    pub wtd: Option<NaiveTime>,
    /// Delay value (in minutes) implied by a change to the service's route.
    ///
    /// This value has been added to the forecast lateness of the service at
    /// the previous schedule location when calculating the expected lateness
    /// of arrival at this location.
    #[builder(default)]
    pub rdelay_mins: i32,
}
schedule_loc_deserialize! { LocDt, LocDtBuilder, ret, value,
    sla sla,
    cpa cpa,
    time wta on ret,
    time_opt wtd on ret,
    parse rdelay_mins from rdelay on ret,
}
/// Operational destination ('OPDT') element.
#[derive(Builder, Debug, Clone)]
#[builder(private)]
pub struct LocOpDt {
    /// Common attributes for this schedule location.
    pub sla: SchedLocAttributes,
    /// Working scheduled time of arrival.
    pub wta: NaiveTime,
    /// Working scheduled time of departure.
    #[builder(default)]
    pub wtd: Option<NaiveTime>,
    /// Delay value (in minutes) implied by a change to the service's route.
    ///
    /// This value has been added to the forecast lateness of the service at
    /// the previous schedule location when calculating the expected lateness
    /// of arrival at this location.
    #[builder(default)]
    pub rdelay_mins: i32,
}
schedule_loc_deserialize! { LocOpDt, LocOpDtBuilder, ret, value,
    sla sla,
    time wta on ret,
    time_opt wtd on ret,
    parse rdelay_mins from rdelay on ret,
}
/// A Darwin train schedule.
#[derive(Builder, Debug, Clone)]
#[builder(private)]
pub struct Schedule {
    /// RTTI unique Train ID.
    pub rid: String,
    /// Train UID.
    pub uid: String,
    /// Train ID (headcode).
    pub train_id: String,
    /// Scheduled start date.
    pub ssd: NaiveDate,
    /// ATOC code.
    pub toc: String,
    /// Type of service, i.e. Train/Bus/Ship.
    #[builder(default = "\"P\".into()")]
    pub status: String,
    /// Category of service.
    #[builder(default = "\"OO\".into()")]
    pub train_cat: String,
    /// Whether Darwin classifies the train category as a passenger service.
    #[builder(default = "true")]
    pub is_passenger_svc: bool,
    /// Indicates if this service is active in Darwin.
    ///
    /// Note that schedules should be assumed to be inactive until a message
    /// is received to indicate otherwise.
    ///
    /// *[Editor's note: this field defaults to true, so probably disregard
    /// the earlier statement for most cases?]*
    #[builder(default = "true")]
    pub is_active: bool,
    /// Whether the service has been deleted, and should not be used or displayed.
    #[builder(default)]
    pub deleted: bool,
    /// Whether the service is a charter service.
    #[builder(default)]
    pub is_charter: bool,
    /// Presumably some form of cancellation reason?
    #[builder(default)]
    pub cancel_reason: Option<DisruptionReason>,
    /// Locations in this schedule.
    pub locations: Vec<ScheduleLocation>,
}
impl XmlDeserialize for Schedule {
    fn from_xml_iter<R: Read>(se: XmlStartElement, reader: &mut EventReader<R>) -> Result<Self> {
        let mut ret = ScheduleBuilder::default();
        xml_attrs! { se, value,
            parse rid, uid, train_id from trainId, toc, status, train_cat from trainCat, is_passenger_svc from isPassengerSvc, is_active from isActive, deleted, is_charter from isCharter on ret,
            with ssd on ret {
                NaiveDate::parse_from_str(&value, "%Y-%m-%d")?
            },
        }
        let mut locs = vec![];
        xml_iter! { se, reader,
            parse "OR", LocOr as or {
                locs.push(ScheduleLocation::Or(or));
            },
            parse "OPOR", LocOpOr as opor {
                locs.push(ScheduleLocation::OpOr(opor));
            },
            parse "IP", LocIp as ip {
                locs.push(ScheduleLocation::Ip(ip));
            },
            parse "OPIP", LocOpIp as opip {
                locs.push(ScheduleLocation::OpIp(opip));
            },
            parse "PP", LocPp as pp {
                locs.push(ScheduleLocation::Pp(pp));
            },
            parse "DT", LocDt as dt {
                locs.push(ScheduleLocation::Dt(dt));
            },
            parse "OPDT", LocOpDt as opdt {
                locs.push(ScheduleLocation::OpDt(opdt));
            },
            parse "cancelReason", DisruptionReason as cr {
                ret.cancel_reason(Some(cr));
            },
        }
        ret.locations(locs);
        xml_build!(ret);
        Ok(ret)
    }
}
/// Notification that a train schedule is deactivated in Darwin.
#[derive(Builder, Debug, Clone)]
#[builder(private)]
pub struct DeactivatedSchedule {
    /// RTTI unique Train ID.
    pub rid: String
}
impl XmlDeserialize for DeactivatedSchedule {
    fn from_xml_iter<R: Read>(se: XmlStartElement, reader: &mut EventReader<R>) -> Result<Self> {
        let mut ret = DeactivatedScheduleBuilder::default();
        xml_attrs! { se, value,
            parse rid on ret,
        }
        xml_iter! { se, reader, }
        xml_build!(ret);
        Ok(ret)
    }
}
