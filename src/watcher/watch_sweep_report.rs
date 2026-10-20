use super::WatcherMessageId;

#[derive(Clone, Debug, Eq, PartialEq, facet::Facet)]
pub struct WatchSweepReport {
    pub baseline_created: bool,
    pub pages_read: u32,
    pub queued: u32,
    pub checkpoint: Option<WatcherMessageId>,
    pub gap: bool,
    pub pending: u64,
}
