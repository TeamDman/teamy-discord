use super::WatcherMessage;

/// The source message ID is the permanent dispatch idempotency key.
#[derive(Clone, Debug, Eq, PartialEq, facet::Facet)]
pub struct WatchOutboxEntry {
    pub message: WatcherMessage,
    /// Nonsecret external receipt identifier. Absence means replay on restart.
    pub delivered_receipt: Option<String>,
}
