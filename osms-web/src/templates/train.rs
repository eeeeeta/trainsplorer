use schedules;

#[derive(Serialize)]
pub struct TrainView {
    pub movements: Vec<TrainMvtDesc>,
    pub trust_id: Option<String>,
    pub parent_sched: i32,
    pub date: String,
    pub nre_id: Option<String>,
    pub parent_nre_sched: Option<i32>,
    pub sched_uid: String,
    pub terminated: bool,
    pub cancelled: bool,
    pub signalling_id: Option<String>,
    pub orig_dest: schedules::ScheduleOrigDest,
    pub darwin_only: bool
}

#[derive(Serialize)]
pub struct TrainMvtDesc {
    pub action: &'static str,
    pub action_past_tense: &'static str,
    pub location: String,
    pub tiploc: String,
    pub time_scheduled: String,
    pub time_expected: Option<String>,
    pub time_actual: Option<String>,
    pub ends_path: Option<i32>,
    pub starts_path: Option<i32>,
    pub delayed: bool,
    pub platform: Option<String>,
    pub pfm_changed: bool,
    pub pfm_suppr: bool,
}
