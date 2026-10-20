use super::ConsumerError;
use super::ConsumerName;
use crate::watcher::WatcherScope;
use sha2::Digest;
use sha2::Sha256;
use std::fmt;
use std::str::FromStr;

/// Deterministic scope/consumer/sequence identity. It is never an expiring lease.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, facet::Facet)]
#[facet(transparent)]
pub struct ConsumerDeliveryId(String);

impl ConsumerDeliveryId {
    /// # Errors
    /// Requires the versioned delivery format `d1-` followed by 64 lowercase hex digits.
    pub fn new(value: impl Into<String>) -> eyre::Result<Self> {
        let value = value.into();
        eyre::ensure!(
            value.len() == 67
                && value.starts_with("d1-")
                && value.as_bytes()[3..]
                    .iter()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte)),
            ConsumerError::InvalidIdentifier
        );
        Ok(Self(value))
    }

    pub(super) fn for_event(scope: &WatcherScope, consumer: &ConsumerName, sequence: u64) -> Self {
        let mut digest = Sha256::new();
        digest.update(b"teamy-discord-consumer-delivery-v1\0");
        for id in [
            scope.guild_id.get(),
            scope.channel_id.get(),
            scope.author_id.get(),
            scope.bot_id.get(),
            sequence,
        ] {
            digest.update(id.to_be_bytes());
        }
        digest.update(consumer.as_str().as_bytes());
        Self(format!("d1-{:x}", digest.finalize()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ConsumerDeliveryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for ConsumerDeliveryId {
    type Err = eyre::Report;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}
