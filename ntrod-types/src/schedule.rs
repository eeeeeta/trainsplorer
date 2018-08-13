use chrono::*;
use super::fns::*;
use super::cif::*;
use chrono_tz::Tz;
use std::fmt;

#[derive(Deserialize, Clone, Debug, is_enum_variant)]
pub enum Record {
    #[serde(rename = "JsonScheduleV1")]
    Schedule(ScheduleRecord),
    #[serde(rename = "JsonAssociationV1")]
    Association(AssociationRecord),
    #[serde(rename = "JsonTimetableV1")]
    Timetable(TimetableRecord),
    #[serde(rename = "TiplocV1")]
    Tiploc(TiplocRecord),
    #[serde(rename = "EOF")]
    Eof(bool)
}
#[derive(Serialize, Deserialize, Copy, Clone, Debug, is_enum_variant)]
#[cfg_attr(feature = "postgres-traits", derive(FromSql, ToSql))]
pub enum CreateOrDelete {
    Create,
    Delete
}
#[derive(Serialize, Deserialize, Default, Copy, Clone, Debug)]
#[cfg_attr(feature = "postgres-traits", derive(FromSql, ToSql))]
pub struct Days {
    pub mon: bool,
    pub tue: bool,
    pub wed: bool,
    pub thu: bool,
    pub fri: bool,
    pub sat: bool,
    pub sun: bool
}
impl fmt::Display for Days {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}{}{}{}{}{}",
               if self.mon { "Mo" } else { "" },
               if self.tue { "Tu" } else { "" },
               if self.wed { "We" } else { "" },
               if self.thu { "Th" } else { "" },
               if self.fri { "Fr" } else { "" },
               if self.sat { "Sa" } else { "" },
               if self.sun { "Su" } else { "" })
    }
}
impl Days {
    pub fn all() -> Self {
        Days {
            mon: true,
            tue: true,
            wed: true,
            thu: true,
            fri: true,
            sat: true,
            sun: true
        }
    }
    pub fn value_for_iso_weekday(&self, wd: u32) -> Option<bool> {
        match wd {
            1 => Some(self.mon),
            2 => Some(self.tue),
            3 => Some(self.wed),
            4 => Some(self.thu),
            5 => Some(self.fri),
            6 => Some(self.sat),
            7 => Some(self.sun),
            _ => None
        }
    }
    pub fn create_type() -> &'static str {
r#"
DO $$
BEGIN
IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'Days') THEN
CREATE TYPE "Days" AS (
mon BOOL,
tue BOOL,
wed BOOL,
thu BOOL,
fri BOOL,
sat BOOL,
sun BOOL
);
END IF;
END$$;"#
    }
    pub fn create_value_for_iso_weekday_function() -> &'static str {
r#"
CREATE OR REPLACE FUNCTION days_value_for_iso_weekday(days "Days", wd int)
    RETURNS boolean AS $$
DECLARE
    ret boolean := false;
BEGIN
    CASE wd
        WHEN 1 THEN
            ret := days.mon;
        WHEN 2 THEN
            ret := days.tue;
        WHEN 3 THEN
            ret := days.wed;
        WHEN 4 THEN
            ret := days.thu;
        WHEN 5 THEN
            ret := days.fri;
        WHEN 6 THEN
            ret := days.sat;
        WHEN 7 THEN
            ret := days.sun;
        ELSE
            RAISE EXCEPTION 'must provide a valid ISO weekday';
        END CASE;
        RETURN ret;
END;
$$ LANGUAGE plpgsql;
"#
    }
}
#[derive(Serialize, Deserialize, Copy, Clone, Debug, SmartDefault, is_enum_variant)]
#[cfg_attr(feature = "postgres-traits", derive(FromSql, ToSql))]
pub enum YesOrNo {
    #[default]
    Y,
    N
}
#[derive(Serialize, Deserialize, Copy, Clone, Debug, is_enum_variant)]
#[cfg_attr(feature = "postgres-traits", derive(FromSql, ToSql))]
pub enum RecordIdentity {
    #[serde(rename = "LO")]
    Originating,
    #[serde(rename = "LI")]
    Intermediate,
    #[serde(rename = "LT")]
    Terminating
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, is_enum_variant)]
#[cfg_attr(feature = "postgres-traits", derive(FromSql, ToSql))]
pub enum AssociationType {
    #[serde(rename = "JJ")]
    Join,
    #[serde(rename = "VV")]
    Divide,
    #[serde(rename = "NP")]
    Next,
    #[serde(rename = "  ")]
    None
}
#[derive(Serialize, Deserialize, Copy, Clone, Debug, is_enum_variant)]
#[cfg_attr(feature = "postgres-traits", derive(FromSql, ToSql))]
pub enum DateIndicator {
    #[serde(rename = "S")]
    Standard,
    #[serde(rename = "N")]
    NextMidnight,
    #[serde(rename = "P")]
    PrevMidnight
}
#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub enum CreateType {
    Create
}
#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub enum DeleteType {
    Delete
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[cfg_attr(feature = "postgres-traits", derive(FromSql, ToSql))]
pub struct Sender {
    pub organisation: String,
    pub application: String,
    pub component: String,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[cfg_attr(feature = "postgres-traits", derive(FromSql, ToSql))]
pub struct TimetableMetadata {
    #[serde(rename = "type", deserialize_with = "non_empty_str")]
    pub ty: String,
    pub sequence: u32
}

#[derive(Serialize, Deserialize, Clone, Debug)]
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
// FIXMEs (c.f. schedule_assoc_delete_1.json):
// - the DateTime isn't parsed like the other ones; why?
// - diagram_type field is strangely missing
#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum AssociationRecord {
    Create {
        transaction_type: CreateType,
        #[serde(deserialize_with = "non_empty_str")]
        main_train_uid: String,
        #[serde(deserialize_with = "non_empty_str")]
        assoc_train_uid: String,
        assoc_start_date: DateTime<Utc>,
        assoc_end_date: DateTime<Utc>,
        #[serde(deserialize_with = "parse_days")]
        assoc_days: Days,
        category: AssociationType,
        location: String,
        #[serde(deserialize_with = "non_empty_str_opt")]
        base_location_suffix: Option<String>,
        #[serde(deserialize_with = "non_empty_str_opt")]
        assoc_location_suffix: Option<String>,
        #[serde(rename = "CIF_stp_indicator")]
        stp_indicator: StpIndicator,
    },
    Delete {
        transaction_type: DeleteType,
        #[serde(deserialize_with = "non_empty_str")]
        main_train_uid: String,
        #[serde(deserialize_with = "non_empty_str")]
        assoc_train_uid: String,
        assoc_start_date: DateTime<Utc>,
        location: String,
        #[serde(deserialize_with = "non_empty_str_opt")]
        base_location_suffix: Option<String>,
        #[serde(rename = "CIF_stp_indicator")]
        stp_indicator: StpIndicator,
    }
}
#[derive(Deserialize, Clone, Debug)]
pub struct ScheduleSegment {
    #[serde(rename = "CIF_train_category")]
    pub train_category: Option<TrainCategory>,
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub signalling_id: Option<String>,
    #[serde(rename = "CIF_headcode", deserialize_with = "non_empty_str_opt")]
    pub headcode: Option<String>,
    #[serde(rename = "CIF_business_sector", deserialize_with = "non_empty_str_opt")]
    pub business_sector: Option<String>,
    #[serde(rename = "CIF_power_type")]
    pub power_type: Option<PowerType>,
    #[serde(rename = "CIF_timing_load", deserialize_with = "non_empty_str_opt")]
    pub timing_load: Option<String>,
    #[serde(rename = "CIF_speed", deserialize_with = "from_str_opt")]
    pub speed: Option<u32>,
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
    #[serde(default)]
    pub schedule_location: Vec<LocationRecord>
}
#[derive(Deserialize, Clone, Debug)]
pub struct TiplocRecord {
    pub transaction_type: CreateOrDelete,
    #[serde(deserialize_with = "non_empty_str")]
    pub tiploc_code: String,
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub nalco: Option<String>,
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub stanox: Option<String>,
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub crs_code: Option<String>,
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub description: Option<String>,
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub tps_description: Option<String>
}
#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum ScheduleRecord {
    Create {
        #[serde(rename = "CIF_train_uid", deserialize_with = "non_empty_str")]
        train_uid: String,
        transaction_type: CreateType,
        #[serde(deserialize_with = "naive_date_to_london")]
        schedule_start_date: Date<Tz>,
        #[serde(deserialize_with = "naive_date_to_london")]
        schedule_end_date: Date<Tz>,
        #[serde(deserialize_with = "parse_days")]
        schedule_days_runs: Days,
        #[serde(rename = "CIF_bank_holiday_running", deserialize_with = "non_empty_str_opt")]
        bank_holiday_running: Option<String>,
        train_status: TrainStatus,
        #[serde(rename = "CIF_stp_indicator")]
        stp_indicator: StpIndicator,
        #[serde(default)]
        applicable_timetable: YesOrNo,
        #[serde(default, deserialize_with = "non_empty_str_opt")]
        atoc_code: Option<String>,
        schedule_segment: ScheduleSegment,
    },
    Delete {
        #[serde(rename = "CIF_train_uid", deserialize_with = "non_empty_str")]
        train_uid: String,
        transaction_type: DeleteType,
        #[serde(deserialize_with = "naive_date_to_london")]
        schedule_start_date: Date<Tz>,
        #[serde(rename = "CIF_stp_indicator")]
        stp_indicator: StpIndicator,
    }
}
#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub enum OriginatingLocation {
    #[serde(rename = "LO")]
    Originating
}
#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub enum IntermediateLocation {
    #[serde(rename = "LI")]
    Intermediate
}
#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub enum TerminatingLocation {
    #[serde(rename = "LT")]
    Terminating
}


#[derive(Deserialize, Clone, Debug, is_enum_variant)]
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
        #[serde(deserialize_with = "non_empty_str_opt")]
        platform: Option<String>,
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
        #[serde(deserialize_with = "non_empty_str_opt")]
        platform: Option<String>,
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
        #[serde(deserialize_with = "non_empty_str_opt")]
        platform: Option<String>,
        #[serde(deserialize_with = "non_empty_str_opt")]
        path: Option<String>,
    }
}
