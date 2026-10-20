use super::BotTokenSource;
use super::ResolvedBotToken;

/// Resolve one explicit input. No file source exists.
/// # Errors
/// Rejects missing, blank, invalid or ambiguous inputs, including equal values.
pub fn resolve_bot_token_from_sources(
    command_line: Option<&str>,
    environment: Option<&str>,
    legacy_environment: Option<&str>,
) -> eyre::Result<ResolvedBotToken> {
    crate::discord::credential_selection::select(
        command_line,
        environment,
        legacy_environment,
        None,
        None,
    )?;
    let value = command_line
        .or(environment)
        .or(legacy_environment)
        .ok_or_else(|| {
            eyre::eyre!("Explicit bot token required; use --bot-token or TEAMY_DISCORD_BOT_TOKEN")
        })?;
    Ok(ResolvedBotToken {
        token: value.trim().to_owned(),
        source: if command_line.is_some() {
            BotTokenSource::CommandLine
        } else {
            BotTokenSource::Environment
        },
    })
}

/// # Errors
/// Rejects invalid environment values or ambiguous explicit inputs. Never reads or saves tokens.
pub fn resolve_bot_token(command_line: Option<&str>) -> eyre::Result<ResolvedBotToken> {
    let environment = |key: &str| -> eyre::Result<Option<String>> {
        match std::env::var(key) {
            Ok(value) => Ok(Some(value)),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(_redacted_error) => eyre::bail!("Invalid credential environment"),
        }
    };
    let token = zeroize::Zeroizing::new(environment(super::BOT_TOKEN_ENV_VAR)?);
    let legacy = zeroize::Zeroizing::new(environment(super::LEGACY_BOT_TOKEN_ENV_VAR)?);
    resolve_bot_token_from_sources(command_line, token.as_deref(), legacy.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_inputs_reject_conflicts_and_invalid_values_without_fallback() {
        for (cli, env, legacy) in [
            (Some("same"), Some("same"), None),
            (None, Some("same"), Some("same")),
            (Some(" "), None, None),
            (Some("bad\nvalue"), None, None),
            (None, None, None),
        ] {
            let error = resolve_bot_token_from_sources(cli, env, legacy).unwrap_err();
            assert!(!format!("{error:?}").contains("bad\nvalue"));
        }
    }
    #[test]
    fn one_explicit_input_is_selected_and_debug_is_redacted() {
        for (cli, env, source) in [
            (Some("synthetic-secret"), None, BotTokenSource::CommandLine),
            (None, Some("synthetic-secret"), BotTokenSource::Environment),
        ] {
            let resolved = resolve_bot_token_from_sources(cli, env, None).unwrap();
            assert_eq!(resolved.source, source);
            assert!(!format!("{resolved:?}").contains("synthetic-secret"));
        }
    }
}
