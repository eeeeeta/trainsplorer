use serde_derive::Serialize;

#[derive(Serialize)]
pub struct MovementSearchView {
    pub error: Option<String>,
    pub station: Option<String>,
    pub date: String,
    pub time: String
}
