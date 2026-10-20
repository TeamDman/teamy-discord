use crate::watcher::WatcherMessage;

#[derive(Clone, Debug, Eq, PartialEq, facet::Facet)]
pub struct ConsumerEvent {
    pub sequence: u64,
    pub message: WatcherMessage,
}
