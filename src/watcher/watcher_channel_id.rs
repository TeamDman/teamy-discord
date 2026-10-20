use std::fmt;
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, facet::Facet)]
#[facet(transparent)]
pub struct WatcherChannelId(u64);

impl WatcherChannelId {
    /// # Errors
    /// Rejects the reserved zero identifier.
    pub fn new(value: u64) -> eyre::Result<Self> {
        eyre::ensure!(value != 0, "A watcher channel ID must be nonzero");
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for WatcherChannelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for WatcherChannelId {
    type Err = eyre::Report;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value.parse()?)
    }
}
