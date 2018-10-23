#[derive(Serialize)]
pub struct TrainView {
    pub movements: Vec<TrainMvtDesc>
}

#[derive(Serialize)]
pub struct TrainMvtDesc {
    pub action: &'static str,
    pub location: String,
    pub tiploc: String,
    pub time_scheduled: String,
    pub time_expected: Option<String>,
    pub time_actual: Option<String>,
    pub ends_path: Option<i32>,
    pub starts_path: Option<i32>
}
