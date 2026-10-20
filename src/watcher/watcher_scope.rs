use super::WatcherChannelId;
use super::WatcherError;
use super::WatcherGuildId;
use super::WatcherUserId;

/// The sole channel and human author authorized to trigger this watcher.
#[derive(Clone, Debug, Eq, PartialEq, facet::Facet)]
pub struct WatcherScope {
    pub guild_id: WatcherGuildId,
    pub channel_id: WatcherChannelId,
    pub author_id: WatcherUserId,
    pub bot_id: WatcherUserId,
}

impl WatcherScope {
    /// # Errors
    /// Rejects invalid IDs and a bot configured as the human trigger author.
    pub fn validate(&self) -> eyre::Result<()> {
        WatcherGuildId::new(self.guild_id.get())?;
        WatcherChannelId::new(self.channel_id.get())?;
        WatcherUserId::new(self.author_id.get())?;
        WatcherUserId::new(self.bot_id.get())?;
        eyre::ensure!(self.author_id != self.bot_id, WatcherError::InvalidSettings);
        Ok(())
    }
}
