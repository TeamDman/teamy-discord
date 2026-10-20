use super::credential_options::CredentialOptions;
use super::credential_selection::CredentialSelection;
use super::leased_http::LeasedHttp;
use eyre::Result;
#[cfg(windows)]
use teamy_broker_sdk::Backend;
#[cfg(windows)]
use teamy_broker_sdk::Client;
use zeroize::Zeroizing;
fn environment(name: &str) -> Result<Option<String>> {
    match std::env::var(name) {
        Ok(v) => Ok(Some(v)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(_redacted_error) => eyre::bail!("Invalid credential environment"),
    }
}
fn metadata(cli: Option<&str>, name: &str) -> Result<Option<String>> {
    metadata_with_default(cli, name, None)
}
fn metadata_with_default(
    cli: Option<&str>,
    name: &str,
    profile: Option<&str>,
) -> Result<Option<String>> {
    let env = environment(name)?;
    crate::paths::broker_reference::resolve_field(cli, env.as_deref(), profile)
}
impl CredentialOptions {
    /// # Errors
    /// Returns redacted errors on conflicts, missing configuration, incompatible broker or denied approval.
    pub async fn connect(
        &self,
        cli_token: Option<&str>,
        operation: &str,
        guild: Option<u64>,
        channel: Option<u64>,
    ) -> Result<LeasedHttp> {
        self.connect_with_profile(cli_token, operation, guild, channel, None)
            .await
    }
    /// Profile fields are defaults only; explicit CLI/environment conflicts are never overridden.
    /// # Errors
    /// Returns safe errors for invalid metadata, conflicts, denial or expiry.
    pub async fn connect_with_profile(
        &self,
        cli_token: Option<&str>,
        operation: &str,
        guild: Option<u64>,
        channel: Option<u64>,
        profile: Option<&crate::paths::BrokerReference>,
    ) -> Result<LeasedHttp> {
        #[cfg(not(windows))]
        let _ = (operation, guild, channel, profile);
        let token = Zeroizing::new(environment("TEAMY_DISCORD_BOT_TOKEN")?);
        let legacy = Zeroizing::new(environment("TEAMY_DISCORD_ARCHIVE_DISCORD_BOT_TOKEN")?);
        let provider = environment("TEAMY_DISCORD_CREDENTIAL_PROVIDER")?;
        let selected = super::credential_selection::select(
            cli_token,
            token.as_deref(),
            legacy.as_deref(),
            self.credential_provider.as_deref(),
            provider.as_deref(),
        )?;
        match selected {
            CredentialSelection::Direct => {
                eyre::ensure!(
                    metadata(
                        self.broker_account.as_deref(),
                        "TEAMY_DISCORD_BROKER_ACCOUNT"
                    )?
                    .is_none()
                        && metadata(
                            self.broker_secret_ref.as_deref(),
                            "TEAMY_DISCORD_BROKER_SECRET_REF"
                        )?
                        .is_none()
                        && metadata(
                            self.credential_lease_id.as_deref(),
                            "TEAMY_DISCORD_CREDENTIAL_LEASE_ID"
                        )?
                        .is_none()
                        && metadata(
                            self.credential_target.as_deref(),
                            "TEAMY_DISCORD_CREDENTIAL_TARGET"
                        )?
                        .is_none()
                        && metadata(
                            self.broker_executable.as_deref(),
                            "TEAMY_DISCORD_BROKER_EXECUTABLE"
                        )?
                        .is_none(),
                    "Token and broker configuration both selected"
                );
                LeasedHttp::direct(
                    cli_token
                        .or(token.as_deref())
                        .or(legacy.as_deref())
                        .unwrap_or_default(),
                )
            }
            #[cfg(windows)]
            CredentialSelection::Provider("teamy-broker") => {
                eyre::ensure!(
                    metadata(
                        self.credential_target.as_deref(),
                        "TEAMY_DISCORD_CREDENTIAL_TARGET"
                    )?
                    .is_none()
                        && metadata(
                            self.credential_lease_id.as_deref(),
                            "TEAMY_DISCORD_CREDENTIAL_LEASE_ID"
                        )?
                        .is_none(),
                    "Memory provider cannot select a persistent cache"
                );
                let account = metadata_with_default(
                    self.broker_account.as_deref(),
                    "TEAMY_DISCORD_BROKER_ACCOUNT",
                    profile.and_then(|p| p.account.as_deref()),
                )?
                .ok_or(super::configuration_error::ConfigurationError::MissingAccount)?;
                crate::paths::broker_reference::validate_account(&account)?;
                let secret_ref = metadata_with_default(
                    self.broker_secret_ref.as_deref(),
                    "TEAMY_DISCORD_BROKER_SECRET_REF",
                    profile.and_then(|p| p.secret_ref.as_deref()),
                )?
                .ok_or(super::configuration_error::ConfigurationError::MissingReference)?;
                crate::paths::broker_reference::validate_reference(&secret_ref)?;
                let executable = metadata(
                    self.broker_executable.as_deref(),
                    "TEAMY_DISCORD_BROKER_EXECUTABLE",
                )?
                .unwrap_or_else(|| "teamy-broker.exe".into());
                let client = Client::new(Some(executable.into()))?;
                super::broker_handoff::acquire(
                    &client, account, secret_ref, operation, guild, channel,
                )
                .await
            }
            #[cfg(windows)]
            CredentialSelection::Provider(provider) => self.persistent(provider).await,
            #[cfg(not(windows))]
            CredentialSelection::Provider(_) => Err(
                super::configuration_error::ConfigurationError::UnsupportedBrokerTransport.into(),
            ),
        }
    }

    #[cfg(windows)]
    async fn persistent(&self, provider: &str) -> Result<LeasedHttp> {
        eyre::ensure!(
            metadata(
                self.broker_account.as_deref(),
                "TEAMY_DISCORD_BROKER_ACCOUNT"
            )?
            .is_none()
                && metadata(
                    self.broker_secret_ref.as_deref(),
                    "TEAMY_DISCORD_BROKER_SECRET_REF"
                )?
                .is_none()
                && metadata(
                    self.broker_executable.as_deref(),
                    "TEAMY_DISCORD_BROKER_EXECUTABLE"
                )?
                .is_none(),
            "Persistent provider cannot select new item approval"
        );
        let id = metadata(
            self.credential_lease_id.as_deref(),
            "TEAMY_DISCORD_CREDENTIAL_LEASE_ID",
        )?
        .ok_or_else(|| eyre::eyre!("Persistent lease ID required"))?;
        let target = metadata(
            self.credential_target.as_deref(),
            "TEAMY_DISCORD_CREDENTIAL_TARGET",
        )?
        .ok_or_else(|| eyre::eyre!("Persistent target required"))?;
        let backend = match provider {
            "windows-credential-manager" => Backend::WindowsCredentialManager(target),
            "well-known-file" => Backend::WellKnownFile(target),
            _ => eyre::bail!("Invalid provider"),
        };
        LeasedHttp::broker(Client::new(None)?.load_cached(backend, id).await?)
    }
}
