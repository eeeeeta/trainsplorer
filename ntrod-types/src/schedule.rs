use chrono::*;
use super::fns::*;
use super::cif::*;

#[derive(Serialize, Deserialize, Debug)]
pub enum Record {
    #[serde(rename = "JsonScheduleV1")]
    Schedule(ScheduleRecord),
    #[serde(rename = "JsonAssociationV1")]
    Association(AssociationRecord),
    #[serde(rename = "JsonTimetableV1")]
    Timetable(TimetableRecord),
}
#[derive(Serialize, Deserialize, Debug)]
pub enum CreateOrDelete {
    Create,
    Delete
}
#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Days {
    pub mon: bool,
    pub tue: bool,
    pub wed: bool,
    pub thu: bool,
    pub fri: bool,
    pub sat: bool,
    pub sun: bool
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
pub enum AssociationType {
    #[serde(rename = "JJ")]
    Join,
    #[serde(rename = "VV")]
    Divide,
    #[serde(rename = "NP")]
    Next
}
#[derive(Serialize, Deserialize, Debug)]
pub enum DateIndicator {
    #[serde(rename = "S")]
    Standard,
    #[serde(rename = "N")]
    NextMidnight,
    #[serde(rename = "P")]
    PrevMidnight
}
#[derive(Serialize, Deserialize, Debug)]
pub struct Sender {
    pub organisation: String,
    pub application: String,
    pub component: String,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct TimetableMetadata {
    #[serde(rename = "type", deserialize_with = "non_empty_str")]
    pub ty: String,
    pub sequence: u32
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TimetableRecord {
    #[serde(deserialize_with = "non_empty_str")]
    pub classification: String,
    pub timestamp: u32,
    #[serde(deserialize_with = "non_empty_str")]
    pub owner: String,
    #[serde(rename = "Sender")]
    pub sender: Sender,
    #[serde(rename = "Metadata")]
    pub metadata: TimetableMetadata
}
#[derive(Serialize, Deserialize, Debug)]
pub struct AssociationRecord {
    pub transaction_type: CreateOrDelete,
    #[serde(deserialize_with = "non_empty_str")]
    pub main_train_uid: String,
    #[serde(deserialize_with = "non_empty_str")]
    pub assoc_train_uid: String,
    pub assoc_start_date: DateTime<Utc>,
    pub assoc_end_date: DateTime<Utc>,
    #[serde(deserialize_with = "parse_days")]
    pub assoc_days: Days,
    pub category: AssociationType,
    pub location: String,
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub base_location_suffix: Option<String>,
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub assoc_location_suffix: Option<String>,
    #[serde(rename = "CIF_stp_indicator")]
    pub stp_indicator: StpIndicator,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct ScheduleSegment {
    #[serde(rename = "CIF_train_category")]
    pub train_category: TrainCategory,
    #[serde(deserialize_with = "non_empty_str")]
    pub signalling_id: String,
    #[serde(rename = "CIF_headcode", deserialize_with = "non_empty_str_opt")]
    pub headcode: Option<String>,
    #[serde(rename = "CIF_business_sector", deserialize_with = "non_empty_str")]
    pub business_sector: String,
    #[serde(rename = "CIF_power_type")]
    pub power_type: PowerType,
    #[serde(rename = "CIF_timing_load", deserialize_with = "non_empty_str")]
    pub timing_load: String,
    #[serde(rename = "CIF_speed", deserialize_with = "from_str")]
    pub speed: u32,
    #[serde(rename = "CIF_operating_characteristics", deserialize_with = "non_empty_str")]
    pub operating_characteristics: String,
    #[serde(rename = "CIF_train_class", deserialize_with = "non_empty_str_opt")]
    pub train_class: Option<String>,
    #[serde(rename = "CIF_sleepers", deserialize_with = "non_empty_str_opt")]
    pub sleepers: Option<String>,
    #[serde(rename = "CIF_reservations", deserialize_with = "non_empty_str_opt")]
    pub reservations: Option<String>,
    #[serde(rename = "CIF_catering_code", deserialize_with = "non_empty_str_opt")]
    pub catering_code: Option<String>,
    #[serde(rename = "CIF_service_branding", deserialize_with = "non_empty_str_opt")]
    pub service_branding: Option<String>,
    pub schedule_location: Vec<LocationRecord>
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ScheduleRecord {
    #[serde(rename = "CIF_train_uid", deserialize_with = "non_empty_str")]
    pub train_uid: String,
    pub transaction_type: CreateOrDelete,
    pub schedule_start_date: NaiveDate,
    pub schedule_end_date: NaiveDate,
    #[serde(deserialize_with = "parse_days")]
    pub schedule_days_runs: Days,
    #[serde(rename = "CIF_bank_holiday_running", deserialize_with = "non_empty_str_opt")]
    pub bank_holiday_running: Option<String>,
    pub train_status: TrainStatus,
    #[serde(rename = "CIF_stp_indicator")]
    pub stp_indicator: StpIndicator,
    pub applicable_timetable: YesOrNo,
    pub schedule_segment: ScheduleSegment,
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
        #[serde(deserialize_with = "non_empty_str")]
        tiploc_code: String,
        #[serde(deserialize_with = "parse_24h_time_force")]
        departure: NaiveTime,
        #[serde(deserialize_with = "parse_24h_time")]
        public_departure: Option<NaiveTime>,
        #[serde(deserialize_with = "from_str_opt")]
        platform: Option<u32>,
        #[serde(deserialize_with = "non_empty_str_opt")]
        line: Option<String>,
        #[serde(deserialize_with = "non_empty_str_opt")]
        engineering_allowance: Option<String>,
        #[serde(deserialize_with = "non_empty_str_opt")]
        pathing_allowance: Option<String>,
        #[serde(deserialize_with = "non_empty_str_opt")]
        performance_allowance: Option<String>
    },
    Intermediate {
        record_identity: IntermediateLocation,
        #[serde(deserialize_with = "non_empty_str")]
        tiploc_code: String,
        #[serde(deserialize_with = "parse_24h_time_force")]
        arrival: NaiveTime,
        #[serde(deserialize_with = "parse_24h_time_force")]
        departure: NaiveTime,
        #[serde(deserialize_with = "parse_24h_time")]
        public_arrival: Option<NaiveTime>,
        #[serde(deserialize_with = "parse_24h_time")]
        public_departure: Option<NaiveTime>,
        #[serde(deserialize_with = "from_str_opt")]
        platform: Option<u32>,
        #[serde(deserialize_with = "non_empty_str_opt")]
        line: Option<String>,
        #[serde(deserialize_with = "non_empty_str_opt")]
        path: Option<String>,
        #[serde(deserialize_with = "non_empty_str_opt")]
        engineering_allowance: Option<String>,
        #[serde(deserialize_with = "non_empty_str_opt")]
        pathing_allowance: Option<String>,
        #[serde(deserialize_with = "non_empty_str_opt")]
        performance_allowance: Option<String>
    },
    Pass {
        record_identity: IntermediateLocation,
        #[serde(deserialize_with = "non_empty_str")]
        tiploc_code: String,
        #[serde(deserialize_with = "parse_24h_time_force")]
        pass: NaiveTime,
        #[serde(deserialize_with = "non_empty_str_opt")]
        engineering_allowance: Option<String>,
        #[serde(deserialize_with = "non_empty_str_opt")]
        pathing_allowance: Option<String>,
        #[serde(deserialize_with = "non_empty_str_opt")]
        performance_allowance: Option<String>
    },
    Terminating {
        record_identity: TerminatingLocation,
        #[serde(deserialize_with = "non_empty_str")]
        tiploc_code: String,
        #[serde(deserialize_with = "parse_24h_time_force")]
        arrival: NaiveTime,
        #[serde(deserialize_with = "parse_24h_time")]
        public_arrival: Option<NaiveTime>,
        #[serde(deserialize_with = "from_str_opt")]
        platform: Option<u32>,
        #[serde(deserialize_with = "non_empty_str_opt")]
        path: Option<String>,
    }
}
