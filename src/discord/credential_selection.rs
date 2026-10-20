use eyre::Result;
/// Consumer-owned raw input conflicts are rejected even when equal or blank.
#[derive(Debug)]
pub enum CredentialSelection {
    Direct,
    Provider(&'static str),
}
/// # Errors
/// Rejects conflicting or invalid explicit credential inputs. Never reads saved preferences.
// cli[impl auth.live-token.ambiguous-inputs-rejected]
// cli[impl auth.live-token.broker-memory-default]
pub fn select(
    cli_token: Option<&str>,
    env_token: Option<&str>,
    legacy_token: Option<&str>,
    cli_provider: Option<&str>,
    env_provider: Option<&str>,
) -> Result<CredentialSelection> {
    let count = [cli_token, env_token, legacy_token]
        .iter()
        .filter(|v| v.is_some())
        .count();
    eyre::ensure!(count <= 1, "Conflicting credential inputs");
    if count == 1 {
        eyre::ensure!(
            cli_provider.is_none() && env_provider.is_none(),
            "Token and provider both selected"
        );
        let token = cli_token.or(env_token).or(legacy_token).unwrap_or_default();
        eyre::ensure!(!token.trim().is_empty(), "Explicit credential is empty");
        eyre::ensure!(
            !token
                .trim()
                .chars()
                .any(|c| c.is_control() || c.is_whitespace()),
            "Invalid explicit bot token"
        );
        return Ok(CredentialSelection::Direct);
    }
    let default = (cli_provider.is_none() && env_provider.is_none()).then_some("teamy-broker");
    Ok(CredentialSelection::Provider(resolve_provider(
        cli_provider.or(default),
        env_provider,
    )?))
}

#[cfg(windows)]
use teamy_broker_sdk::resolve_provider;
#[cfg(not(windows))]
fn resolve_provider(cli: Option<&str>, environment: Option<&str>) -> Result<&'static str> {
    let selected = match (cli, environment) {
        (Some(_), Some(_)) => eyre::bail!("CLI and environment both select a provider"),
        (None, None) => eyre::bail!("an explicit provider is required"),
        (Some(value), None) | (None, Some(value)) => value,
    };
    match selected {
        "teamy-broker" => Ok("teamy-broker"),
        "windows-credential-manager" => Ok("windows-credential-manager"),
        "well-known-file" => Ok("well-known-file"),
        _ => eyre::bail!("invalid explicit provider"),
    }
}
