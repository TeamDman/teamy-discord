use super::BotTokenSource;
#[derive(Clone, PartialEq, Eq)]
pub struct ResolvedBotToken {
    pub token: String,
    pub source: BotTokenSource,
}
impl std::fmt::Debug for ResolvedBotToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedBotToken")
            .field("token", &"[REDACTED]")
            .field("source", &self.source)
            .finish()
    }
}
