use crate::watcher::WatcherSource;
/// Authenticated, read-only capture source. It owns no persistent state.
/// The outer production runner must also stop credential-bearing Gateway tasks at expiry.
pub trait DaemonCaptureSource: WatcherSource {
    /// # Errors
    /// Rejects a fixed expired lease. Must never renew or acquire a credential.
    fn check_lease(&self) -> eyre::Result<()>;
}
