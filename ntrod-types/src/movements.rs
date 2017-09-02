use chrono::*;
use super::cif::*;
use super::fns::*;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MvtHeader {
    pub msg_type: String,
    pub source_dev_id: String,
    pub source_system_id: String,
    pub original_data_source: String
}

pub type Records = Vec<Record>;
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Record {
    pub header: MvtHeader,
    pub body: MvtBody
}
#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub enum ScheduleSource {
    #[serde(rename = "C")]
    CifItps,
    #[serde(rename = "V")]
    VstpTops
}
#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub enum AutomaticOrManual {
    #[serde(rename = "AUTOMATIC")]
    Automatic,
    #[serde(rename = "MANUAL")]
    Manual
}
#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub enum CallMode {
    #[serde(rename = "NORMAL")]
    Normal,
    #[serde(rename = "OVERNIGHT")]
    Overnight
}
#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub enum EventType {
    #[serde(rename = "ARRIVAL")]
    Arrival,
    #[serde(rename = "DEPARTURE")]
    Departure,
    #[serde(rename = "DESTINATION")]
    Destination
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub enum CanxType {
    #[serde(rename = "ON CALL")]
    OnActivation,
    #[serde(rename = "AT ORIGIN")]
    AtOrigin,
    #[serde(rename = "EN ROUTE")]
    EnRoute,
    #[serde(rename = "OUT OF PLAN")]
    OffRoute
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub enum VariationStatus {
    #[serde(rename = "ON TIME")]
    OnTime,
    #[serde(rename = "EARLY")]
    Early,
    #[serde(rename = "LATE")]
    Late,
    #[serde(rename = "OFF ROUTE")]
    OffRoute
}
#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub enum UpOrDown {
    #[serde(rename = "UP")]
    Up,
    #[serde(rename = "DOWN")]
    Down,
    #[serde(rename = "")]
    None
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum MvtBody {
    Activation(Activation),
    Cancellation(Cancellation),
    Movement(Movement),
    Reinstatement(Reinstatement),
    ChangeOfOrigin(ChangeOfOrigin),
    ChangeOfIdentity(ChangeOfIdentity)
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Activation {
    pub schedule_source: ScheduleSource,
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub train_file_address: Option<String>,
    #[serde(deserialize_with = "from_str")]
    pub schedule_end_date: NaiveDate,
    #[serde(deserialize_with = "non_empty_str")]
    pub train_id: String,
    #[serde(deserialize_with = "from_str")]
    pub tp_origin_timestamp: NaiveDate,
    #[serde(deserialize_with = "parse_ts")]
    pub creation_timestamp: NaiveDateTime,
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub tp_origin_stanox: Option<String>,
    #[serde(deserialize_with = "parse_ts")]
    pub origin_dep_timestamp: NaiveDateTime,
    #[serde(deserialize_with = "non_empty_str")]
    pub train_service_code: String,
    #[serde(deserialize_with = "non_empty_str")]
    pub toc_id: String,
    #[serde(deserialize_with = "non_empty_str")]
    pub d1266_record_number: String,
    pub train_call_type: AutomaticOrManual,
    #[serde(deserialize_with = "non_empty_str")]
    pub train_uid: String,
    pub train_call_mode: CallMode,
    #[serde(deserialize_with = "fix_buggy_schedule_type")]
    pub schedule_type: StpIndicator,
    #[serde(deserialize_with = "non_empty_str")]
    pub sched_origin_stanox: String,
    #[serde(deserialize_with = "non_empty_str")]
    pub schedule_wtt_id: String,
    #[serde(deserialize_with = "from_str")]
    pub schedule_start_date: NaiveDate
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Cancellation {
    #[serde(deserialize_with = "non_empty_str")]
    pub train_service_code: String,
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub train_file_address: Option<String>,
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub orig_loc_stanox: Option<String>,
    #[serde(deserialize_with = "non_empty_str")]
    pub toc_id: String,
    #[serde(deserialize_with = "parse_ts")]
    pub dep_timestamp: NaiveDateTime,
    #[serde(deserialize_with = "non_empty_str")]
    pub division_code: String,
    #[serde(deserialize_with = "non_empty_str")]
    pub loc_stanox: String,
    #[serde(deserialize_with = "parse_ts")]
    pub canx_timestamp: NaiveDateTime,
    #[serde(deserialize_with = "non_empty_str")]
    pub canx_reason_code: String,
    #[serde(deserialize_with = "non_empty_str")]
    pub train_id: String,
    #[serde(deserialize_with = "parse_ts_opt")]
    pub orig_loc_timestamp: Option<NaiveDateTime>,
    pub canx_type: CanxType
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Movement {
    pub event_type: EventType,
    #[serde(deserialize_with = "parse_ts_opt")]
    pub gbtt_timestamp: Option<NaiveDateTime>,
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub original_loc_stanox: Option<String>,
    #[serde(deserialize_with = "parse_ts_opt")]
    pub planned_timestamp: Option<NaiveDateTime>,
    #[serde(deserialize_with = "from_str_trimming")]
    pub timetable_variation: u32,
    #[serde(deserialize_with = "parse_ts_opt")]
    pub original_loc_timestamp: Option<NaiveDateTime>,
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub current_train_id: Option<String>,
    #[serde(deserialize_with = "from_str")]
    pub delay_monitoring_point: bool,
    #[serde(deserialize_with = "from_str_opt_trimming")]
    pub next_report_run_time: Option<u32>,
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub reporting_stanox: Option<String>,
    #[serde(deserialize_with = "parse_ts")]
    pub actual_timestamp: NaiveDateTime,
    #[serde(deserialize_with = "from_str")]
    pub correction_ind: bool,
    pub event_source: AutomaticOrManual,
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub train_file_address: Option<String>,
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub platform: Option<String>,
    #[serde(deserialize_with = "non_empty_str")]
    pub division_code: String,
    #[serde(deserialize_with = "from_str")]
    pub train_terminated: bool,
    #[serde(deserialize_with = "non_empty_str")]
    pub train_id: String,
    #[serde(deserialize_with = "from_str")]
    pub offroute_ind: bool,
    pub variation_status: VariationStatus,
    #[serde(deserialize_with = "non_empty_str")]
    pub train_service_code: String,
    #[serde(deserialize_with = "non_empty_str")]
    pub toc_id: String,
    #[serde(deserialize_with = "non_empty_str")]
    pub loc_stanox: String,
    #[serde(deserialize_with = "from_str_opt")]
    pub auto_expected: Option<bool>,
    pub direction_ind: UpOrDown,
    #[serde(deserialize_with = "from_str_opt")]
    pub route: Option<char>,
    pub planned_event_type: EventType,
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub next_report_stanox: Option<String>,
    #[serde(deserialize_with = "from_str_opt")]
    pub line_ind: Option<char>
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Reinstatement {
    #[serde(deserialize_with = "non_empty_str")]
    pub train_id: String,
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub current_train_id: Option<String>,
    #[serde(deserialize_with = "parse_ts_opt")]
    pub original_loc_timestamp: Option<NaiveDateTime>,
    #[serde(deserialize_with = "parse_ts")]
    pub dep_timestamp: NaiveDateTime,
    #[serde(deserialize_with = "non_empty_str")]
    pub loc_stanox: String,
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub original_loc_stanox: Option<String>,
    #[serde(deserialize_with = "parse_ts")]
    pub reinstatement_timestamp: NaiveDateTime,
    #[serde(deserialize_with = "non_empty_str")]
    pub toc_id: String,
    #[serde(deserialize_with = "non_empty_str")]
    pub division_code: String,
    #[serde(deserialize_with = "non_empty_str")]
    pub train_service_code: String
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChangeOfOrigin {
    #[serde(deserialize_with = "non_empty_str")]
    pub train_id: String,
    #[serde(deserialize_with = "parse_ts")]
    pub dep_timestamp: NaiveDateTime,
    #[serde(deserialize_with = "non_empty_str")]
    pub loc_stanox: String,
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub original_loc_stanox: Option<String>,
    #[serde(deserialize_with = "parse_ts_opt")]
    pub original_loc_timestamp: Option<NaiveDateTime>,
    #[serde(deserialize_with = "non_empty_str")]
    pub train_service_code: String,
    #[serde(deserialize_with = "non_empty_str")]
    pub reason_code: String,
    #[serde(deserialize_with = "non_empty_str")]
    pub division_code: String,
    #[serde(deserialize_with = "non_empty_str")]
    pub toc_id: String,
    #[serde(deserialize_with = "parse_ts")]
    pub coo_timestamp: NaiveDateTime
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChangeOfIdentity {
    #[serde(deserialize_with = "non_empty_str")]
    pub train_id: String,
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub current_train_id: Option<String>,
    #[serde(deserialize_with = "non_empty_str")]
    pub revised_train_id: String,
    #[serde(deserialize_with = "non_empty_str")]
    pub train_service_code: String,
    #[serde(deserialize_with = "parse_ts")]
    pub event_timestamp: NaiveDateTime
}
