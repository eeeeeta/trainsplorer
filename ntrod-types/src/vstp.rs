use super::schedule::{CreateOrDelete, YesOrNo, Days};
use super::cif::*;
use super::fns::*;
use chrono::*;

#[derive(Serialize, Deserialize, Debug)]
pub enum Record {
    #[serde(rename = "VSTPCIFMsgV1")]
    V1(VstpMessage)
}
#[derive(Serialize, Deserialize, Debug)]
pub struct VstpMessage {
    #[serde(deserialize_with = "non_empty_str")]
    pub timestamp: String,
    #[serde(deserialize_with = "non_empty_str")]
    pub owner: String,
    #[serde(deserialize_with = "non_empty_str")]
    pub classification: String,
    #[serde(rename = "originMsgId", deserialize_with = "non_empty_str")]
    pub origin_msg_id: String,
    pub schedule: VstpScheduleRecord
}
#[derive(Serialize, Deserialize, Debug)]
pub struct VstpScheduleRecord {
    #[serde(rename = "CIF_train_uid", deserialize_with = "non_empty_str")]
    pub train_uid: String,
    pub transaction_type: CreateOrDelete,
    pub schedule_start_date: NaiveDate,
    pub schedule_end_date: NaiveDate,
    #[serde(deserialize_with = "parse_days")]
    pub schedule_days_runs: Days,
    #[serde(rename = "CIF_bank_holiday_running")]
    pub bank_holiday_running: Option<String>,
    pub train_status: TrainStatus,
    #[serde(rename = "CIF_stp_indicator")]
    pub stp_indicator: StpIndicator,
    pub applicable_timetable: YesOrNo,
    pub schedule_segment: Vec<VstpScheduleSegment>
}
#[derive(Serialize, Deserialize, Debug)]
pub struct VstpScheduleSegment {
    #[serde(rename = "CIF_train_category")]
    pub train_category: TrainCategory,
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub signalling_id: Option<String>,
    #[serde(rename = "CIF_headcode", deserialize_with = "non_empty_str_opt")]
    pub headcode: Option<String>,
    #[serde(rename = "CIF_business_sector", deserialize_with = "non_empty_str_opt")]
    pub business_sector: Option<String>,
    #[serde(rename = "CIF_power_type")]
    pub power_type: PowerType,
    #[serde(rename = "CIF_timing_load", deserialize_with = "non_empty_str_opt")]
    pub timing_load: Option<String>,
    #[serde(rename = "CIF_speed", deserialize_with = "non_empty_str_opt")]
    pub speed: Option<String>,
    #[serde(rename = "CIF_operating_characteristics", deserialize_with = "non_empty_str_opt")]
    pub operating_characteristics: Option<String>,
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
}
