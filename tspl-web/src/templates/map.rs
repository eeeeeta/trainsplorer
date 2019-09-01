use serde_derive::Serialize;

#[derive(Serialize)]
pub struct ProblemStation {
    pub name: String,
    pub stanox: Option<String>,
    pub latlon: String
}
#[derive(Serialize)]
pub struct MapView {
    pub problem_stations: Vec<ProblemStation>,
    pub n_stations: usize
}
