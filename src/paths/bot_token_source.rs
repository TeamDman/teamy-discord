#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BotTokenSource {
    CommandLine,
    Environment,
}
impl BotTokenSource {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::CommandLine => "command-line",
            Self::Environment => "environment",
        }
    }
}
