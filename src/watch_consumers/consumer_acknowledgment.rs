use super::ConsumerDeliveryId;
use super::ConsumerName;
use crate::watcher::WatcherMessageId;

#[derive(Clone, Debug, Eq, PartialEq, facet::Facet)]
pub struct ConsumerAcknowledgment {
    pub consumer: ConsumerName,
    pub event_id: WatcherMessageId,
    pub event_sequence: u64,
    pub delivery_id: ConsumerDeliveryId,
    pub receipt: String,
}
