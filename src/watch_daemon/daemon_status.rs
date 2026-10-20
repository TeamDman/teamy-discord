use crate::watcher::WatcherMessageId;
use crate::watcher::WatcherScope;
#[derive(Clone, Debug, facet::Facet)]
pub struct DaemonStatus {
    pub scope: WatcherScope,
    pub capture: String,
    pub coverage_complete: bool,
    pub checkpoint: Option<WatcherMessageId>,
    pub waiting_consumers: u64,
}
