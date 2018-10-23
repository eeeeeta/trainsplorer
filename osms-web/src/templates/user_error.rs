#[derive(Serialize)]
pub struct UserErrorView {
    pub error_summary: String,
    pub reason: String
}
