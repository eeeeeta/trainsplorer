use schedules;
use templates::movement_search::MovementSearchView;
use chrono::NaiveTime;

#[derive(Serialize)]
pub struct MovementsView {
    pub mvts: Vec<MovementDesc>,
    pub mvt_search: MovementSearchView
}
#[derive(Serialize, Debug)]
pub struct MovementDesc {
    pub parent_sched: i32,
    pub parent_train: Option<i32>,
    pub tiploc: String,
    pub action: &'static str,
    pub time: String,
    pub time_expected: Option<String>,
    pub time_actual: Option<String>,
    pub platform: Option<String>,
    pub pfm_changed: bool,
    pub pfm_suppr: bool,
    pub action_past_tense: &'static str,
    pub delayed: bool,
    pub canx: bool,
    #[serde(skip_serializing)]
    pub _action: i32,
    #[serde(skip_serializing)]
    pub _time: NaiveTime,
    pub orig_dest: Option<schedules::ScheduleOrigDest>,
}
