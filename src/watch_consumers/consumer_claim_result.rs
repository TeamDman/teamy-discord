use super::ConsumerAcknowledgment;
use super::ConsumerClaim;
use super::ConsumerDelivery;

#[derive(Clone, Debug, Eq, PartialEq, facet::Facet)]
#[repr(u8)]
pub enum ConsumerClaimResult {
    Claimed(ConsumerDelivery),
    PendingAck(ConsumerClaim),
    Empty,
    AlreadyCompleted(ConsumerAcknowledgment),
}
