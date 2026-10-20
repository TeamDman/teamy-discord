use arbitrary::Arbitrary;
use facet::Facet;

/// Explicit initial position for a newly created consumer.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
#[repr(u8)]
pub enum WatchConsumerStartCli {
    /// Begin after all events already captured by the daemon.
    Now,
    /// Begin at the oldest retained event.
    Oldest,
}

impl From<WatchConsumerStartCli> for crate::watch_consumers::ConsumerStart {
    fn from(value: WatchConsumerStartCli) -> Self {
        match value {
            WatchConsumerStartCli::Now => Self::Now,
            WatchConsumerStartCli::Oldest => Self::Oldest,
        }
    }
}
