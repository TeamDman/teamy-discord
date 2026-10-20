use crate::watcher::WatcherScope;
/// A fixed deadline allows a waiting client to distinguish daemon expiry from other EOF.
#[derive(Clone, Debug, facet::Facet)]
pub struct DaemonHello {
    pub version: u32,
    pub scope: WatcherScope,
    pub lease_expires_unix_ms: Option<u64>,
}
