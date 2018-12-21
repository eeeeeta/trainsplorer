#[derive(Serialize)]
pub struct ProblemStation {
    pub name: String,
    pub stanox: Option<String>,
    pub latlon: String
}
#[derive(Serialize)]
pub struct MapView {
    pub problem_stations: Vec<ProblemStation>
}
