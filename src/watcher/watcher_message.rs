use super::WatcherChannelId;
use super::WatcherError;
use super::WatcherGuildId;
use super::WatcherMessageId;
use super::WatcherReply;
use super::WatcherScope;
use super::WatcherUserId;

/// Transport-verified Discord identity and content. Text alone cannot forge a mention.
#[derive(Clone, Debug, Eq, PartialEq, facet::Facet)]
pub struct WatcherMessage {
    pub id: WatcherMessageId,
    pub guild_id: WatcherGuildId,
    pub channel_id: WatcherChannelId,
    pub author_id: WatcherUserId,
    pub author_is_bot: bool,
    pub is_webhook: bool,
    pub mentioned_user_ids: Vec<WatcherUserId>,
    pub content: String,
    pub reply_to: Option<WatcherMessageId>,
    pub reply: Option<WatcherReply>,
}

impl WatcherMessage {
    /// # Errors
    /// Rejects a response outside the selected channel or invalid identity.
    pub fn validate_scope(&self, scope: &WatcherScope) -> eyre::Result<()> {
        WatcherMessageId::new(self.id.get())?;
        WatcherUserId::new(self.author_id.get())?;
        eyre::ensure!(self.guild_id == scope.guild_id, WatcherError::ScopeMismatch);
        eyre::ensure!(
            self.channel_id == scope.channel_id,
            WatcherError::ScopeMismatch
        );
        for user_id in &self.mentioned_user_ids {
            WatcherUserId::new(user_id.get())?;
        }
        if let Some(reply_to) = self.reply_to {
            WatcherMessageId::new(reply_to.get())?;
        }
        Ok(())
    }

    /// # Errors
    /// Rejects missing or mismatched fetched context for an otherwise valid trigger.
    pub fn is_trigger(&self, scope: &WatcherScope) -> eyre::Result<bool> {
        self.validate_scope(scope)?;
        if self.author_id != scope.author_id
            || self.author_is_bot
            || self.is_webhook
            || !self.mentioned_user_ids.contains(&scope.bot_id)
        {
            return Ok(false);
        }
        match (self.reply_to, &self.reply) {
            (None, None) => {}
            (Some(expected), Some(reply)) => {
                eyre::ensure!(reply.id == expected, WatcherError::InvalidReply);
                eyre::ensure!(reply.guild_id == scope.guild_id, WatcherError::InvalidReply);
                eyre::ensure!(
                    reply.channel_id == scope.channel_id,
                    WatcherError::InvalidReply
                );
                WatcherUserId::new(reply.author_id.get())?;
            }
            _ => eyre::bail!(WatcherError::InvalidReply),
        }
        Ok(true)
    }

    #[must_use]
    pub fn permalink(&self) -> String {
        format!(
            "https://discord.com/channels/{}/{}/{}",
            self.guild_id, self.channel_id, self.id
        )
    }
}
