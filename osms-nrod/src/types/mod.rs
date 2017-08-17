use chrono::*;

mod fns;
pub mod cif;
#[cfg(test)]
mod tests;

use self::fns::*;
use self::cif::*;

#[derive(Serialize, Deserialize, Debug)]
pub enum Record {
    #[serde(rename = "JsonScheduleV1")]
    Schedule(ScheduleRecord)
}
#[derive(Serialize, Deserialize, Debug)]
pub enum CreateOrDelete {
    Create,
    Delete
}

#[derive(Serialize, Deserialize, Debug)]
pub enum YesOrNo {
    Y,
    N
}
#[derive(Serialize, Deserialize, Debug)]
pub enum RecordIdentity {
    #[serde(rename = "LO")]
    Originating,
    #[serde(rename = "LI")]
    Intermediate,
    #[serde(rename = "LT")]
    Terminating
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ScheduleSegment {
    #[serde(rename = "CIF_train_category")]
    train_category: String,
    signalling_id: String,
    #[serde(rename = "CIF_headcode")]
    headcode: String,
    #[serde(rename = "CIF_business_sector")]
    business_sector: String,
    #[serde(rename = "CIF_power_type")]
    power_type: PowerType,
    #[serde(rename = "CIF_timing_load")]
    timing_load: String,
    #[serde(rename = "CIF_speed")]
    speed: String,
    #[serde(rename = "CIF_operating_characteristics")]
    operating_characteristics: String,
    #[serde(rename = "CIF_train_class")]
    train_class: Option<String>,
    #[serde(rename = "CIF_sleepers")]
    sleepers: Option<String>,
    #[serde(rename = "CIF_reservations")]
    reservations: Option<String>,
    #[serde(rename = "CIF_catering_code")]
    catering_code: Option<String>,
    #[serde(rename = "CIF_service_branding")]
    service_branding: Option<String>,
    schedule_location: Vec<LocationRecord>
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ScheduleRecord {
    #[serde(rename = "CIF_train_uid")]
    train_uid: String,
    transaction_type: CreateOrDelete,
    schedule_start_date: NaiveDate,
    schedule_end_date: NaiveDate,
    schedule_days_runs: String,
    #[serde(rename = "CIF_bank_holiday_running")]
    bank_holiday_running: Option<String>,
    train_status: TrainStatus,
    #[serde(rename = "CIF_stp_indicator")]
    stp_indicator: StpIndicator,
    applicable_timetable: YesOrNo,
    schedule_segment: ScheduleSegment,
}
#[derive(Serialize, Deserialize, Debug)]
pub enum OriginatingLocation {
    #[serde(rename = "LO")]
    Originating
}
#[derive(Serialize, Deserialize, Debug)]
pub enum IntermediateLocation {
    #[serde(rename = "LI")]
    Intermediate
}
#[derive(Serialize, Deserialize, Debug)]
pub enum TerminatingLocation {
    #[serde(rename = "LT")]
    Terminating
}


#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum LocationRecord {
    Originating {
        record_identity: OriginatingLocation,
        tiploc_code: String,
        #[serde(deserialize_with = "parse_24h_time_force")]
        departure: NaiveTime,
        #[serde(deserialize_with = "parse_24h_time")]
        public_departure: Option<NaiveTime>,
        platform: Option<String>,
        line: Option<String>,
        engineering_allowance: Option<String>,
        pathing_allowance: Option<String>,
        performance_allowance: Option<String>
    },
    Intermediate {
        record_identity: IntermediateLocation,
        tiploc_code: String,
        #[serde(deserialize_with = "parse_24h_time_force")]
        arrival: NaiveTime,
        #[serde(deserialize_with = "parse_24h_time_force")]
        departure: NaiveTime,
        #[serde(deserialize_with = "parse_24h_time")]
        public_arrival: Option<NaiveTime>,
        #[serde(deserialize_with = "parse_24h_time")]
        public_departure: Option<NaiveTime>,
        platform: Option<String>,
        line: Option<String>,
        path: Option<String>,
        engineering_allowance: Option<String>,
        pathing_allowance: Option<String>,
        performance_allowance: Option<String>
    },
    Pass {
        record_identity: IntermediateLocation,
        tiploc_code: String,
        #[serde(deserialize_with = "parse_24h_time_force")]
        pass: NaiveTime,
        engineering_allowance: Option<String>,
        pathing_allowance: Option<String>,
        performance_allowance: Option<String>
    },
    Terminating {
        record_identity: TerminatingLocation,
        tiploc_code: String,
        #[serde(deserialize_with = "parse_24h_time_force")]
        arrival: NaiveTime,
        #[serde(deserialize_with = "parse_24h_time")]
        public_arrival: Option<NaiveTime>,
        platform: Option<u32>,
        path: Option<String>,
    }
}
