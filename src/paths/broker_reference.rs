use facet::Facet;
/// Nonsecret per-profile defaults. These are item addresses, never credential values.
#[derive(Facet, Clone, Default, PartialEq, Eq)]
#[facet(rename_all = "kebab-case")]
pub struct BrokerReference {
    pub account: Option<String>,
    pub secret_ref: Option<String>,
}
impl std::fmt::Debug for BrokerReference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BrokerReference([nonsecret metadata redacted])")
    }
}
impl BrokerReference {
    /// # Errors
    /// Rejects malformed nonsecret metadata without echoing it.
    pub fn validate(&self) -> eyre::Result<()> {
        if let Some(account) = &self.account {
            validate_account(account)?;
        }
        if let Some(reference) = &self.secret_ref {
            validate_reference(reference)?;
        }
        Ok(())
    }
}
/// # Errors
/// Requires an explicit 1Password account UUID, email or sign-in address.
pub fn validate_account(value: &str) -> eyre::Result<()> {
    let uuid = value.len() == 26
        && value
            .bytes()
            .all(|b| b.is_ascii_uppercase() || (b'2'..=b'7').contains(&b));
    let address = value.contains('@')
        || value.ends_with(".1password.com")
        || value.ends_with(".1password.eu")
        || value.ends_with(".1password.ca");
    eyre::ensure!(
        (uuid || address)
            && value.len() <= 1024
            && !value.chars().any(char::is_whitespace)
            && !value.chars().any(char::is_control)
            && !value.starts_with('-'),
        crate::discord::configuration_error::ConfigurationError::InvalidAccount
    );
    Ok(())
}
/// # Errors
/// Requires an exact item-field op reference; never accepts a credential value.
pub fn validate_reference(value: &str) -> eyre::Result<()> {
    eyre::ensure!(
        value.len() <= 1024
            && !value.chars().any(char::is_control)
            && value.strip_prefix("op://").is_some_and(
                |v| v.split('/').count() >= 3 && v.split('/').all(|p| !p.trim().is_empty())
            ),
        crate::discord::configuration_error::ConfigurationError::InvalidReference
    );
    Ok(())
}

/// Profile is a default; two explicit sources conflict even when equal or invalid.
pub(crate) fn resolve_field(
    cli: Option<&str>,
    environment: Option<&str>,
    profile: Option<&str>,
) -> eyre::Result<Option<String>> {
    eyre::ensure!(
        cli.is_none() || environment.is_none(),
        "Conflicting broker metadata inputs"
    );
    Ok(cli.or(environment).or(profile).map(str::to_owned))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn per_field_defaults_do_not_override_explicit_inputs_or_hide_conflicts() {
        assert_eq!(
            resolve_field(None, None, Some("profile"))
                .unwrap()
                .as_deref(),
            Some("profile")
        );
        assert_eq!(
            resolve_field(Some("cli"), None, Some("profile"))
                .unwrap()
                .as_deref(),
            Some("cli")
        );
        assert_eq!(
            resolve_field(None, Some("env"), Some("profile"))
                .unwrap()
                .as_deref(),
            Some("env")
        );
        let _ = resolve_field(Some("equal"), Some("equal"), Some("profile")).unwrap_err();
        assert_eq!(
            resolve_field(Some(""), None, Some("profile"))
                .unwrap()
                .as_deref(),
            Some("")
        );
    }
    #[test]
    fn invalid_addresses_never_become_credentials_or_silent_defaults() {
        assert!(validate_account("help").is_err());
        assert!(validate_account("--help").is_err());
        validate_account("AAAAAAAAAAAAAAAAAAAAAAAAAA").unwrap();
        assert!(validate_reference("TOKEN_SENTINEL_NOT_AN_ADDRESS").is_err());
        validate_reference("op://Synthetic/Bot/credential").unwrap();
        assert!(validate_reference("op://Synthetic//credential").is_err());
    }
}
