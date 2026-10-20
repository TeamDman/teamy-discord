use super::WatcherMessageId;

#[derive(Clone, Debug, Eq, PartialEq, facet::Facet)]
pub struct WatchSweepResume {
    pub captured_head: WatcherMessageId,
    pub before: WatcherMessageId,
    pub lower_checkpoint: Option<WatcherMessageId>,
    pub pages_completed: u64,
}
