/// Reviewed configuration failures; no arbitrary values or error chains are rendered.
#[derive(Debug)]
pub enum ConfigurationError {
    UnsupportedBrokerTransport,
    MissingGuild,
    MissingChannel,
    MissingAccount,
    MissingReference,
    InvalidAccount,
    InvalidReference,
}
impl ConfigurationError {
    #[must_use]
    pub const fn message(&self) -> &'static str {
        match self {
            Self::UnsupportedBrokerTransport => {
                "Windows broker transport is unavailable on this platform; inject TEAMY_DISCORD_BOT_TOKEN at runtime"
            }
            Self::MissingGuild => "Provide --guild-id or select a profile with a default guild",
            Self::MissingChannel => {
                "Provide --channel-id/--thread-id or select a profile with a default channel"
            }
            Self::MissingAccount => {
                "Configure profile broker set <name> --account <account>, TEAMY_DISCORD_BROKER_ACCOUNT or --broker-account"
            }
            Self::MissingReference => {
                "Configure profile broker set <name> --secret-ref <op-reference>, TEAMY_DISCORD_BROKER_SECRET_REF or --broker-secret-ref"
            }
            Self::InvalidAccount => {
                "Broker account must be a 1Password account UUID, email or sign-in address"
            }
            Self::InvalidReference => {
                "Broker secret reference must be an exact op://vault/item/field address"
            }
        }
    }
}
impl std::fmt::Display for ConfigurationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}
impl std::error::Error for ConfigurationError {}
