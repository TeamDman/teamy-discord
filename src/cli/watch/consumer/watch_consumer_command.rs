use arbitrary::Arbitrary;
use facet::Facet;

/// Manage named consumers independently of Discord capture.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum WatchConsumerCommand {
    /// Create a cursor at now or the oldest retained event; an existing name keeps its cursor.
    Create(super::create::WatchConsumerCreateArgs),
}
