#[derive(Debug, serde::Serialize)]
pub struct MentionIdentity {
    pub id: u64,
    pub username: String,
    pub bot: bool,
}
