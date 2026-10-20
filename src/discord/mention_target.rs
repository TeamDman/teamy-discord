use super::message_list_error::MessageListError;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MentionTarget {
    AuthenticatedBot,
    User(u64),
}
impl MentionTarget {
    /// # Errors
    /// Rejects unsupported text and zero IDs without echoing input.
    pub fn parse(value: &str) -> eyre::Result<Self> {
        if value == "me" {
            return Ok(Self::AuthenticatedBot);
        }
        let id = value
            .parse::<u64>()
            .map_err(|_redacted_error| MessageListError::InvalidMentions)?;
        eyre::ensure!(id != 0, MessageListError::InvalidMentions);
        Ok(Self::User(id))
    }
    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::AuthenticatedBot => "me".into(),
            Self::User(id) => id.to_string(),
        }
    }
}
