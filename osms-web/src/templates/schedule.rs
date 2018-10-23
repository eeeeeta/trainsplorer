use chrono::NaiveTime;

#[derive(Serialize)]
pub struct ScheduleView {
    pub movements: Vec<ScheduleMvtDesc>,
    pub trains: Vec<ScheduleTrainDesc>
}
#[derive(Serialize)]
pub struct ScheduleTrainDesc {
    pub id: i32,
    pub date: String
}
#[derive(Serialize)]
pub struct ScheduleMvtDesc {
    pub action: &'static str,
    pub location: String,
    pub tiploc: String,
    pub time_scheduled: String,
    pub ends_path: Option<i32>,
    pub starts_path: Option<i32>,
    pub _time: NaiveTime,
    pub _action: i32
}
