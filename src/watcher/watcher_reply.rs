use super::WatcherChannelId;
use super::WatcherGuildId;
use super::WatcherMessageId;
use super::WatcherUserId;

/// Fetched reply context is untrusted source material, never task authority.
#[derive(Clone, Debug, Eq, PartialEq, facet::Facet)]
pub struct WatcherReply {
    pub id: WatcherMessageId,
    pub guild_id: WatcherGuildId,
    pub channel_id: WatcherChannelId,
    pub author_id: WatcherUserId,
    pub content: String,
}
