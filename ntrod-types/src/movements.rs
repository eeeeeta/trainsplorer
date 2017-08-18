use chrono::*;
use super::cif::*;
use super::fns::*;

#[derive(Serialize, Deserialize, Debug)]
pub struct MvtHeader {
    msg_type: String,
    source_dev_id: String,
    source_system_id: String,
    original_data_source: String
}

pub type Records = Vec<Record>;
#[derive(Serialize, Deserialize, Debug)]
pub struct Record {
    header: MvtHeader,
    body: MvtBody
}
#[derive(Serialize, Deserialize, Debug)]
pub enum ScheduleSource {
    #[serde(rename = "C")]
    CifItps,
    #[serde(rename = "V")]
    VstpTops
}
#[derive(Serialize, Deserialize, Debug)]
pub enum AutomaticOrManual {
    #[serde(rename = "AUTOMATIC")]
    Automatic,
    #[serde(rename = "MANUAL")]
    Manual
}
#[derive(Serialize, Deserialize, Debug)]
pub enum CallMode {
    #[serde(rename = "NORMAL")]
    Normal,
    #[serde(rename = "OVERNIGHT")]
    Overnight
}
#[derive(Serialize, Deserialize, Debug)]
pub enum EventType {
    #[serde(rename = "ARRIVAL")]
    Arrival,
    #[serde(rename = "DEPARTURE")]
    Departure,
    #[serde(rename = "DESTINATION")]
    Destination
}

#[derive(Serialize, Deserialize, Debug)]
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

#[derive(Serialize, Deserialize, Debug)]
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
#[derive(Serialize, Deserialize, Debug)]
pub enum UpOrDown {
    #[serde(rename = "UP")]
    Up,
    #[serde(rename = "DOWN")]
    Down,
    #[serde(rename = "")]
    None
}
#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum MvtBody {
    Activation(Activation),
    Cancellation(Cancellation),
    Movement(Movement),
    Reinstatement(Reinstatement),
    ChangeOfOrigin(ChangeOfOrigin),
    ChangeOfIdentity(ChangeOfIdentity)
}
#[derive(Serialize, Deserialize, Debug)]
pub struct Activation {
    schedule_source: ScheduleSource,
    #[serde(deserialize_with = "non_empty_str_opt")]
    train_file_address: Option<String>,
    #[serde(deserialize_with = "from_str")]
    schedule_end_date: NaiveDate,
    #[serde(deserialize_with = "non_empty_str")]
    train_id: String,
    #[serde(deserialize_with = "from_str")]
    tp_origin_timestamp: NaiveDate,
    #[serde(deserialize_with = "parse_ts")]
    creation_timestamp: NaiveDateTime,
    #[serde(deserialize_with = "non_empty_str_opt")]
    tp_origin_stanox: Option<String>,
    #[serde(deserialize_with = "parse_ts")]
    origin_dep_timestamp: NaiveDateTime,
    #[serde(deserialize_with = "non_empty_str")]
    train_service_code: String,
    #[serde(deserialize_with = "non_empty_str")]
    toc_id: String,
    #[serde(deserialize_with = "non_empty_str")]
    d1266_record_number: String,
    train_call_type: AutomaticOrManual,
    #[serde(deserialize_with = "non_empty_str")]
    train_uid: String,
    train_call_mode: CallMode,
    #[serde(deserialize_with = "fix_buggy_schedule_type")]
    schedule_type: StpIndicator,
    #[serde(deserialize_with = "non_empty_str")]
    sched_origin_stanox: String,
    #[serde(deserialize_with = "non_empty_str")]
    schedule_wtt_id: String,
    #[serde(deserialize_with = "from_str")]
    schedule_start_date: NaiveDate
}
#[derive(Serialize, Deserialize, Debug)]
pub struct Cancellation {
    #[serde(deserialize_with = "non_empty_str")]
    train_service_code: String,
    #[serde(deserialize_with = "non_empty_str_opt")]
    train_file_address: Option<String>,
    #[serde(deserialize_with = "non_empty_str_opt")]
    orig_loc_stanox: Option<String>,
    #[serde(deserialize_with = "non_empty_str")]
    toc_id: String,
    #[serde(deserialize_with = "parse_ts")]
    dep_timestamp: NaiveDateTime,
    #[serde(deserialize_with = "non_empty_str")]
    division_code: String,
    #[serde(deserialize_with = "non_empty_str")]
    loc_stanox: String,
    #[serde(deserialize_with = "parse_ts")]
    canx_timestamp: NaiveDateTime,
    #[serde(deserialize_with = "non_empty_str")]
    canx_reason_code: String,
    #[serde(deserialize_with = "non_empty_str")]
    train_id: String,
    #[serde(deserialize_with = "parse_ts_opt")]
    orig_loc_timestamp: Option<NaiveDateTime>,
    canx_type: CanxType
}
#[derive(Serialize, Deserialize, Debug)]
pub struct Movement {
    event_type: EventType,
    #[serde(deserialize_with = "parse_ts_opt")]
    gbtt_timestamp: Option<NaiveDateTime>,
    #[serde(deserialize_with = "non_empty_str_opt")]
    original_loc_stanox: Option<String>,
    #[serde(deserialize_with = "parse_ts_opt")]
    planned_timestamp: Option<NaiveDateTime>,
    #[serde(deserialize_with = "from_str_trimming")]
    timetable_variation: u32,
    #[serde(deserialize_with = "parse_ts_opt")]
    original_loc_timestamp: Option<NaiveDateTime>,
    #[serde(deserialize_with = "non_empty_str_opt")]
    current_train_id: Option<String>,
    #[serde(deserialize_with = "from_str")]
    delay_monitoring_point: bool,
    #[serde(deserialize_with = "from_str_opt_trimming")]
    next_report_run_time: Option<u32>,
    #[serde(deserialize_with = "non_empty_str_opt")]
    reporting_stanox: Option<String>,
    #[serde(deserialize_with = "parse_ts")]
    actual_timestamp: NaiveDateTime,
    #[serde(deserialize_with = "from_str")]
    correction_ind: bool,
    event_source: AutomaticOrManual,
    #[serde(deserialize_with = "non_empty_str_opt")]
    train_file_address: Option<String>,
    #[serde(deserialize_with = "non_empty_str_opt")]
    platform: Option<String>,
    #[serde(deserialize_with = "non_empty_str")]
    division_code: String,
    #[serde(deserialize_with = "from_str")]
    train_terminated: bool,
    #[serde(deserialize_with = "non_empty_str")]
    train_id: String,
    #[serde(deserialize_with = "from_str")]
    offroute_ind: bool,
    variation_status: VariationStatus,
    #[serde(deserialize_with = "non_empty_str")]
    train_service_code: String,
    #[serde(deserialize_with = "non_empty_str")]
    toc_id: String,
    #[serde(deserialize_with = "non_empty_str")]
    loc_stanox: String,
    #[serde(deserialize_with = "from_str_opt")]
    auto_expected: Option<bool>,
    direction_ind: UpOrDown,
    #[serde(deserialize_with = "from_str_opt")]
    route: Option<char>,
    planned_event_type: EventType,
    #[serde(deserialize_with = "non_empty_str_opt")]
    next_report_stanox: Option<String>,
    #[serde(deserialize_with = "from_str_opt")]
    line_ind: Option<char>
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Reinstatement {
    #[serde(deserialize_with = "non_empty_str")]
    train_id: String,
    #[serde(deserialize_with = "non_empty_str_opt")]
    current_train_id: Option<String>,
    #[serde(deserialize_with = "parse_ts_opt")]
    original_loc_timestamp: Option<NaiveDateTime>,
    #[serde(deserialize_with = "parse_ts")]
    dep_timestamp: NaiveDateTime,
    #[serde(deserialize_with = "non_empty_str")]
    loc_stanox: String,
    #[serde(deserialize_with = "non_empty_str_opt")]
    original_loc_stanox: Option<String>,
    #[serde(deserialize_with = "parse_ts")]
    reinstatement_timestamp: NaiveDateTime,
    #[serde(deserialize_with = "non_empty_str")]
    toc_id: String,
    #[serde(deserialize_with = "non_empty_str")]
    division_code: String,
    #[serde(deserialize_with = "non_empty_str")]
    train_service_code: String
}
#[derive(Serialize, Deserialize, Debug)]
pub struct ChangeOfOrigin {
    #[serde(deserialize_with = "non_empty_str")]
    train_id: String,
    #[serde(deserialize_with = "parse_ts")]
    dep_timestamp: NaiveDateTime,
    #[serde(deserialize_with = "non_empty_str")]
    loc_stanox: String,
    #[serde(deserialize_with = "non_empty_str_opt")]
    original_loc_stanox: Option<String>,
    #[serde(deserialize_with = "parse_ts_opt")]
    original_loc_timestamp: Option<NaiveDateTime>,
    #[serde(deserialize_with = "non_empty_str")]
    train_service_code: String,
    #[serde(deserialize_with = "non_empty_str")]
    reason_code: String,
    #[serde(deserialize_with = "non_empty_str")]
    division_code: String,
    #[serde(deserialize_with = "non_empty_str")]
    toc_id: String,
    #[serde(deserialize_with = "parse_ts")]
    coo_timestamp: NaiveDateTime
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ChangeOfIdentity {
    #[serde(deserialize_with = "non_empty_str")]
    train_id: String,
    #[serde(deserialize_with = "non_empty_str_opt")]
    current_train_id: Option<String>,
    #[serde(deserialize_with = "non_empty_str")]
    revised_train_id: String,
    #[serde(deserialize_with = "non_empty_str")]
    train_service_code: String,
    #[serde(deserialize_with = "parse_ts")]
    event_timestamp: NaiveDateTime
}
