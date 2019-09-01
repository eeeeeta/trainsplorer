use serde_derive::Serialize;

#[derive(Serialize)]
pub struct NotFoundView {
    pub uri: String
}
