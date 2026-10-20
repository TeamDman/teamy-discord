use super::mention_target::MentionTarget;
use super::message_list_error::MessageListError;
#[derive(Debug)]
pub struct MessageListRequest {
    pub channel_id: u64,
    pub limit: u8,
    pub before: Option<u64>,
    pub mentions: Option<MentionTarget>,
    pub scan_limit: u16,
}
impl MessageListRequest {
    /// # Errors
    /// Validates a fixed scan ceiling and typed mention selection before credentials.
    pub fn new(
        channel_id: u64,
        limit: Option<u8>,
        before: Option<u64>,
        mentions: Option<&str>,
        scan_limit: Option<u16>,
    ) -> eyre::Result<Self> {
        let mentions = mentions.map(MentionTarget::parse).transpose()?;
        eyre::ensure!(
            mentions.is_some() || scan_limit.is_none(),
            MessageListError::ScanWithoutMentions
        );
        let scan_limit = scan_limit.unwrap_or(100);
        eyre::ensure!(
            (1..=1000).contains(&scan_limit),
            MessageListError::InvalidScanLimit
        );
        Ok(Self {
            channel_id,
            limit: super::live::normalize_message_limit(limit),
            before,
            mentions,
            scan_limit,
        })
    }
}
