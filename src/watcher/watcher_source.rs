use super::WatcherMessage;
use super::WatcherMessageId;

/// Read-only transport adapter. It verifies the authenticated bot and channel
/// membership, and hydrates replies on qualifying human mentions before returning.
/// Pages must be strictly newest-first and strictly older than `before`.
pub trait WatcherSource {
    fn page(
        &mut self,
        before: Option<WatcherMessageId>,
        limit: u8,
    ) -> impl std::future::Future<Output = eyre::Result<Vec<WatcherMessage>>> + Send;

    fn get(
        &mut self,
        id: WatcherMessageId,
    ) -> impl std::future::Future<Output = eyre::Result<WatcherMessage>> + Send;
}
