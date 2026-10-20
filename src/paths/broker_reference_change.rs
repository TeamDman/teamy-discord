use super::AppHome;
use super::BrokerReference;
use super::Profiles;
/// Typed local metadata mutations. No vault, daemon or credential operation is represented.
pub enum BrokerReferenceChange {
    Set {
        name: String,
        account: Option<String>,
        secret_ref: Option<String>,
    },
    Clear {
        name: String,
        account: bool,
        secret_ref: bool,
        all: bool,
    },
}
impl std::fmt::Debug for BrokerReferenceChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BrokerReferenceChange([nonsecret metadata redacted])")
    }
}
impl BrokerReferenceChange {
    /// # Errors
    /// Validates a typed mutation and publishes it under the existing profile lock; omitted fields survive.
    pub fn apply(self, home: &AppHome) -> eyre::Result<Option<BrokerReference>> {
        let name = match &self {
            Self::Set { name, .. } | Self::Clear { name, .. } => name.clone(),
        };
        let profiles = Profiles::update(home, |profiles| {
            let index = profiles
                .profiles
                .iter()
                .position(|p| p.name == name)
                .ok_or_else(|| eyre::eyre!("Unknown profile; use profile list"))?;
            let target = &mut profiles.profiles[index].broker;
            match self {
                Self::Set {
                    account,
                    secret_ref,
                    ..
                } => {
                    eyre::ensure!(
                        account.is_some() || secret_ref.is_some(),
                        "Provide --account and/or --secret-ref for profile broker set"
                    );
                    BrokerReference {
                        account: account.clone(),
                        secret_ref: secret_ref.clone(),
                    }
                    .validate()?;
                    let broker = target.get_or_insert_with(Default::default);
                    if let Some(account) = account {
                        broker.account = Some(account);
                    }
                    if let Some(reference) = secret_ref {
                        broker.secret_ref = Some(reference);
                    }
                }
                Self::Clear {
                    account,
                    secret_ref,
                    all,
                    ..
                } => {
                    eyre::ensure!(
                        (all && !account && !secret_ref) || (!all && (account || secret_ref)),
                        "Choose --all or --account/--secret-ref for profile broker clear"
                    );
                    if all {
                        *target = None;
                    } else if let Some(fields) = target {
                        if account {
                            fields.account = None;
                        }
                        if secret_ref {
                            fields.secret_ref = None;
                        }
                        if fields.account.is_none() && fields.secret_ref.is_none() {
                            *target = None;
                        }
                    }
                }
            }
            Ok(())
        })?;
        Ok(profiles.named(&name)?.broker.clone())
    }
}
