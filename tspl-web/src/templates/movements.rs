use crate::templates::movement_search::MovementSearchView;
use serde_derive::Serialize;
use chrono::NaiveTime;

#[derive(Serialize, Debug, Clone)]
pub struct MovementOrigDest {
    pub orig: String,
    pub dest: String
}
#[derive(Serialize)]
pub struct MovementsView {
    pub mvts: Vec<MovementDesc>,
    pub mvt_search: MovementSearchView
}
#[derive(Serialize, Debug)]
pub struct MovementDesc {
    pub parent_sched: Option<String>,
    pub parent_train: Option<String>,
    pub tiploc: String,
    pub action: &'static str,
    pub time: String,
    pub time_scheduled: Option<String>,
    pub actual: bool,
    pub platform: Option<String>,
    pub pfm_changed: bool,
    pub pfm_suppr: bool,
    pub action_past_tense: &'static str,
    pub delayed: bool,
    pub canx: bool,
    pub orig_dest: MovementOrigDest,
    #[serde(skip_serializing)]
    pub _time: NaiveTime,
    #[serde(skip_serializing)]
    pub _action: u8
}
