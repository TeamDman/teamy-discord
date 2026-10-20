#[derive(Debug, serde::Serialize)]
pub struct MessageListReport {
    pub messages: Vec<serenity::all::Message>,
    pub mentions_user_id: u64,
    pub authenticated_bot: Option<super::mention_identity::MentionIdentity>,
    pub limit: u8,
    pub scan_limit: u16,
    pub fetched: usize,
    pub scanned: usize,
    pub returned: usize,
    pub stop_reason: &'static str,
    pub next_before: Option<u64>,
}
