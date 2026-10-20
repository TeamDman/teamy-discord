#[derive(Debug)]
pub enum MessageListError {
    InvalidMentions,
    InvalidScanLimit,
    ScanWithoutMentions,
    NotBot,
    InvalidPage,
}
impl std::fmt::Display for MessageListError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}
impl std::error::Error for MessageListError {}
impl MessageListError {
    #[must_use]
    pub const fn message(&self) -> &'static str {
        match self {
            Self::InvalidMentions => {
                "Invalid --mentions: use me for the authenticated bot or a nonzero numeric user ID; use message list --help"
            }
            Self::InvalidScanLimit => {
                "Invalid --scan-limit: use 1..1000 messages; use message list --help"
            }
            Self::ScanWithoutMentions => {
                "--scan-limit requires --mentions; use message list --help"
            }
            Self::NotBot => {
                "--mentions me requires an authenticated bot account; owner identity is never substituted"
            }
            Self::InvalidPage => {
                "Discord returned a non-progressing or invalid message page; bounded scan stopped"
            }
        }
    }
}
