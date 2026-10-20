use super::ConsumerClaim;
use crate::watcher::WatcherMessage;

#[derive(Clone, Debug, Eq, PartialEq, facet::Facet)]
pub struct ConsumerDelivery {
    pub claim: ConsumerClaim,
    pub message: WatcherMessage,
}
