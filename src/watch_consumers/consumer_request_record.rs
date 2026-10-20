use super::ConsumerAcknowledgment;
use super::ConsumerClaim;
use super::ConsumerRequestBinding;
use super::ConsumerRequestId;

#[derive(Clone, Debug, Eq, PartialEq, facet::Facet)]
pub struct ConsumerRequestRecord {
    pub id: ConsumerRequestId,
    pub binding: ConsumerRequestBinding,
    pub claim: Option<ConsumerClaim>,
    pub completed_ack: Option<ConsumerAcknowledgment>,
}
