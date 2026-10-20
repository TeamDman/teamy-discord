use super::mention_identity::MentionIdentity;
/// Read-only adapter seam. Implementations must enforce credential leases on every request.
pub trait MessagePageSource {
    fn current_user(&self) -> impl Future<Output = eyre::Result<MentionIdentity>> + Send;
    fn page(
        &self,
        channel: u64,
        before: Option<u64>,
        limit: u8,
    ) -> impl Future<Output = eyre::Result<Vec<serenity::all::Message>>> + Send;
}
