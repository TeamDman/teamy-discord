use super::ConsumerDeliveryId;
use super::ConsumerName;
use super::ConsumerRequestId;
use crate::watcher::WatcherMessageId;

/// Durable in-flight work, never reassigned or released by socket closure.
#[derive(Clone, Debug, Eq, PartialEq, facet::Facet)]
pub struct ConsumerClaim {
    pub consumer: ConsumerName,
    pub event_id: WatcherMessageId,
    pub event_sequence: u64,
    pub delivery_id: ConsumerDeliveryId,
    pub request_id: ConsumerRequestId,
}
