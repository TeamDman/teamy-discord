use super::ConsumerClaim;
use super::ConsumerName;
use super::ConsumerStart;

#[derive(Clone, Debug, Eq, PartialEq, facet::Facet)]
pub struct ConsumerRegistration {
    pub name: ConsumerName,
    pub start: ConsumerStart,
    pub created_sequence: u64,
    pub cursor_sequence: u64,
    pub in_flight: Option<ConsumerClaim>,
}
