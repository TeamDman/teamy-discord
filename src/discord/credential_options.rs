use arbitrary::Arbitrary;
use facet::Facet;
use figue::{self as args};
/// Nonsecret provider metadata; item references belong in local configuration.
#[derive(Facet, Arbitrary, Default, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct CredentialOptions {
    /// Provider: teamy-broker (memory default), windows-credential-manager, well-known-file.
    #[facet(args::named)]
    pub credential_provider: Option<String>,
    /// Broker executable for safe start-if-absent; defaults to teamy-broker.exe on PATH.
    #[facet(args::named)]
    pub broker_executable: Option<String>,
    /// Explicit 1Password account for new memory leases.
    #[facet(args::named)]
    pub broker_account: Option<String>,
    /// Exact item-field reference for new memory leases.
    #[facet(args::named)]
    pub broker_secret_ref: Option<String>,
    /// Existing persistent provider lease UUID; no new approval or fallback.
    #[facet(args::named)]
    pub credential_lease_id: Option<String>,
    /// Explicit namespaced Credential Manager target or absolute protected-file path.
    #[facet(args::named)]
    pub credential_target: Option<String>,
}
impl std::fmt::Debug for CredentialOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CredentialOptions([redacted configuration])")
    }
}
