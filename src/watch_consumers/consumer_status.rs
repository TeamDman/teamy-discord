use super::ConsumerClaim;
use super::ConsumerName;

#[derive(Clone, Debug, Eq, PartialEq, facet::Facet)]
pub struct ConsumerStatus {
    pub name: ConsumerName,
    pub cursor_sequence: u64,
    pub in_flight: Option<ConsumerClaim>,
    pub last_published_sequence: u64,
}
