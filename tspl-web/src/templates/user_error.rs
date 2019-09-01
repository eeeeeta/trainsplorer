use serde_derive::Serialize;

#[derive(Serialize)]
pub struct UserErrorView {
    pub error_summary: String,
    pub reason: String
}
