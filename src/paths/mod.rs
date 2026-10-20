mod broker_reference_change;
mod profile_update_guard;
pub use broker_reference_change::BrokerReferenceChange;
mod profile;
pub use profile::Profile;
pub(crate) mod broker_reference;
pub use broker_reference::BrokerReference;
mod app_home;
mod bot_token;
mod bot_token_source;
pub mod legacy_token_storage;
mod resolved_bot_token;
pub use bot_token_source::BotTokenSource;
pub use resolved_bot_token::ResolvedBotToken;
mod cache;
mod output_dir;
mod profiles;
mod sync_state;
pub use app_home::*;
pub use bot_token::*;
pub use cache::*;
pub use output_dir::*;
pub use profiles::*;
pub use sync_state::*;

pub const APP_HOME_ENV_VAR: &str = "TEAMY_DISCORD_HOME_DIR";
pub const LEGACY_APP_HOME_ENV_VAR: &str = "TEAMY_DISCORD_ARCHIVE_HOME_DIR";
// Retain the existing physical storage identity; no automatic relocation.
pub const APP_HOME_DIR_NAME: &str = "teamy-discord-archive";
pub const APP_CACHE_ENV_VAR: &str = "TEAMY_DISCORD_CACHE_DIR";
pub const LEGACY_APP_CACHE_ENV_VAR: &str = "TEAMY_DISCORD_ARCHIVE_CACHE_DIR";
pub const APP_CACHE_DIR_NAME: &str = "teamy-discord-archive";
pub const BOT_TOKEN_ENV_VAR: &str = "TEAMY_DISCORD_BOT_TOKEN";
pub const LEGACY_BOT_TOKEN_ENV_VAR: &str = "TEAMY_DISCORD_ARCHIVE_DISCORD_BOT_TOKEN";
pub const OUTPUT_DIR_ENV_VAR: &str = "TEAMY_DISCORD_OUTPUT_DIR";
pub const LEGACY_OUTPUT_DIR_ENV_VAR: &str = "TEAMY_DISCORD_ARCHIVE_OUTPUT_DIR";

/// New nonblank environment values win over legacy ones, with lazy fallback.
#[must_use]
pub fn environment_preference(new: &str, legacy: &str) -> Option<String> {
    let preferred = std::env::var(new).ok();
    if let Some(value) = preferred_environment_value(preferred.as_deref(), None) {
        return Some(value.to_owned());
    }
    let fallback = std::env::var(legacy).ok();
    preferred_environment_value(None, fallback.as_deref()).map(str::to_owned)
}

/// Select a nonblank new value before a legacy fallback, without global mutation.
#[must_use]
pub fn preferred_environment_value<'a>(
    new: Option<&'a str>,
    legacy: Option<&'a str>,
) -> Option<&'a str> {
    new.filter(|value| !value.trim().is_empty())
        .or_else(|| legacy.filter(|value| !value.trim().is_empty()))
}
#[cfg(test)]
mod migration_tests {
    use super::*;
    #[test]
    fn new_nonblank_values_win_without_changing_the_storage_identity() {
        assert_eq!(
            preferred_environment_value(Some("new"), Some("old")),
            Some("new")
        );
        assert_eq!(
            preferred_environment_value(Some(" "), Some("old")),
            Some("old")
        );
        assert_eq!(preferred_environment_value(None, Some("")), None);
        assert_eq!(APP_HOME_DIR_NAME, "teamy-discord-archive");
        assert_eq!(APP_CACHE_DIR_NAME, "teamy-discord-archive");
    }
}
