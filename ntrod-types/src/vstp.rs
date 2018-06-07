use super::schedule::{YesOrNo, Days};
use super::cif::*;
use super::fns::*;
use chrono::*;

#[derive(Deserialize, Debug, is_enum_variant)]
pub enum Record {
    #[serde(rename = "VSTPCIFMsgV1")]
    V1(VstpMessage)
}
#[derive(Deserialize, Debug)]
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
#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub enum CreateType {
    Create
}
#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub enum DeleteType {
    Delete
}
#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum VstpScheduleRecord {
    Create {
        #[serde(rename = "CIF_train_uid", deserialize_with = "non_empty_str")]
        train_uid: String,
        transaction_type: CreateType,
        schedule_start_date: NaiveDate,
        schedule_end_date: NaiveDate,
        #[serde(deserialize_with = "parse_days")]
        schedule_days_runs: Days,
        #[serde(rename = "CIF_bank_holiday_running")]
        bank_holiday_running: Option<String>,
        train_status: TrainStatus,
        #[serde(rename = "CIF_stp_indicator")]
        stp_indicator: StpIndicator,
        applicable_timetable: YesOrNo,
        schedule_segment: Vec<VstpScheduleSegment>
    },
    Delete {
        #[serde(rename = "CIF_train_uid", deserialize_with = "non_empty_str")]
        train_uid: String,
        transaction_type: DeleteType,
        schedule_start_date: NaiveDate,
        schedule_end_date: NaiveDate,
        #[serde(deserialize_with = "parse_days")]
        schedule_days_runs: Days,
        #[serde(rename = "CIF_bank_holiday_running")]
        bank_holiday_running: Option<String>,
        train_status: TrainStatus,
        #[serde(rename = "CIF_stp_indicator")]
        stp_indicator: StpIndicator,
    }
}
#[derive(Deserialize, Clone, Debug)]
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
    pub schedule_location: Vec<VstpLocationRecord>
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VstpObnoxiousLocationTiploc {
    #[serde(deserialize_with = "non_empty_str")]
    pub tiploc_id: String
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VstpObnoxiousLocation {
    pub tiploc: VstpObnoxiousLocationTiploc
}
#[derive(Deserialize, Clone, Debug)]
pub struct VstpLocationRecordPass {
    #[serde(deserialize_with = "parse_vstp_time_force")]
    pub scheduled_pass_time: NaiveTime,
    #[serde(rename = "CIF_platform", deserialize_with = "non_empty_str_opt")]
    pub platform: Option<String>,
    #[serde(rename = "CIF_line", deserialize_with = "non_empty_str_opt")]
    pub line: Option<String>,
    #[serde(rename = "CIF_path", deserialize_with = "non_empty_str_opt")]
    pub path: Option<String>,
    #[serde(rename = "CIF_activity", deserialize_with = "non_empty_str_opt")]
    pub activity: Option<String>,
    #[serde(rename = "CIF_engineering_allowance", deserialize_with = "non_empty_str_opt")]
    pub engineering_allowance: Option<String>,
    #[serde(rename = "CIF_pathing_allowance", deserialize_with = "non_empty_str_opt")]
    pub pathing_allowance: Option<String>,
    #[serde(rename = "CIF_performance_allowance", deserialize_with = "non_empty_str_opt")]
    pub performance_allowance: Option<String>,
    pub location: VstpObnoxiousLocation
}
#[derive(Deserialize, Clone, Debug)]
pub struct VstpLocationRecordIntermediate {
    #[serde(deserialize_with = "parse_vstp_time_force")]
    pub scheduled_departure_time: NaiveTime,
    #[serde(deserialize_with = "parse_vstp_time")]
    pub public_departure_time: Option<NaiveTime>,
    #[serde(deserialize_with = "parse_vstp_time_force")]
    pub scheduled_arrival_time: NaiveTime,
    #[serde(deserialize_with = "parse_vstp_time")]
    pub public_arrival_time: Option<NaiveTime>,
    #[serde(rename = "CIF_platform", deserialize_with = "non_empty_str_opt")]
    pub platform: Option<String>,
    #[serde(rename = "CIF_line", deserialize_with = "non_empty_str_opt")]
    pub line: Option<String>,
    #[serde(rename = "CIF_path", deserialize_with = "non_empty_str_opt")]
    pub path: Option<String>,
    #[serde(rename = "CIF_activity", deserialize_with = "non_empty_str_opt")]
    pub activity: Option<String>,
    #[serde(rename = "CIF_engineering_allowance", deserialize_with = "non_empty_str_opt")]
    pub engineering_allowance: Option<String>,
    #[serde(rename = "CIF_pathing_allowance", deserialize_with = "non_empty_str_opt")]
    pub pathing_allowance: Option<String>,
    #[serde(rename = "CIF_performance_allowance", deserialize_with = "non_empty_str_opt")]
    pub performance_allowance: Option<String>,
    pub location: VstpObnoxiousLocation
}
#[derive(Deserialize, Clone, Debug)]
pub struct VstpLocationRecordOriginating {
    #[serde(deserialize_with = "parse_vstp_time_force")]
    pub scheduled_departure_time: NaiveTime,
    #[serde(deserialize_with = "parse_vstp_time")]
    pub public_departure_time: Option<NaiveTime>,
    #[serde(rename = "CIF_platform", deserialize_with = "non_empty_str_opt")]
    pub platform: Option<String>,
    #[serde(rename = "CIF_line", deserialize_with = "non_empty_str_opt")]
    pub line: Option<String>,
    #[serde(rename = "CIF_path", deserialize_with = "non_empty_str_opt")]
    pub path: Option<String>,
    #[serde(rename = "CIF_activity", deserialize_with = "non_empty_str_opt")]
    pub activity: Option<String>,
    #[serde(rename = "CIF_engineering_allowance", deserialize_with = "non_empty_str_opt")]
    pub engineering_allowance: Option<String>,
    #[serde(rename = "CIF_pathing_allowance", deserialize_with = "non_empty_str_opt")]
    pub pathing_allowance: Option<String>,
    #[serde(rename = "CIF_performance_allowance", deserialize_with = "non_empty_str_opt")]
    pub performance_allowance: Option<String>,
    pub location: VstpObnoxiousLocation
}
#[derive(Deserialize, Clone, Debug)]
pub struct VstpLocationRecordTerminating {
    #[serde(deserialize_with = "parse_vstp_time_force")]
    pub scheduled_arrival_time: NaiveTime,
    #[serde(deserialize_with = "parse_vstp_time")]
    pub public_arrival_time: Option<NaiveTime>,
    #[serde(rename = "CIF_platform", deserialize_with = "non_empty_str_opt")]
    pub platform: Option<String>,
    #[serde(rename = "CIF_line", deserialize_with = "non_empty_str_opt")]
    pub line: Option<String>,
    #[serde(rename = "CIF_path", deserialize_with = "non_empty_str_opt")]
    pub path: Option<String>,
    #[serde(rename = "CIF_activity", deserialize_with = "non_empty_str_opt")]
    pub activity: Option<String>,
    #[serde(rename = "CIF_engineering_allowance", deserialize_with = "non_empty_str_opt")]
    pub engineering_allowance: Option<String>,
    #[serde(rename = "CIF_pathing_allowance", deserialize_with = "non_empty_str_opt")]
    pub pathing_allowance: Option<String>,
    #[serde(rename = "CIF_performance_allowance", deserialize_with = "non_empty_str_opt")]
    pub performance_allowance: Option<String>,
    pub location: VstpObnoxiousLocation
}
#[derive(Deserialize, Clone, Debug, is_enum_variant)]
#[serde(untagged)]
pub enum VstpLocationRecord {
    Pass(VstpLocationRecordPass),
    Intermediate(VstpLocationRecordIntermediate),
    Originating(VstpLocationRecordOriginating),
    Terminating(VstpLocationRecordTerminating)
}
