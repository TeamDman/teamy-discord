use arbitrary::Arbitrary;
use facet::Facet;
use figue::{self as args};
#[derive(Facet, Arbitrary, Default, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct ReadOptions {
    /// Select a named profile; otherwise use the active profile.
    #[facet(args::named)]
    pub profile: Option<String>,
    /// Print local query planning only. Does not read credentials or contact Discord.
    #[facet(args::named, default)]
    pub plan: bool,
    /// Explicit consumer token. Conflicts with any environment token or explicit provider.
    #[facet(args::named)]
    pub bot_token: Option<String>,
    #[facet(flatten, default)]
    pub credential: super::credential_options::CredentialOptions,
    /// Archive root used only for channel/thread metadata evidence.
    #[facet(args::named)]
    pub archive_root: Option<String>,
    /// Nonsecret read policy; otherwise use read-policy.json in app home if present.
    #[facet(args::named)]
    pub read_policy: Option<String>,
}
impl std::fmt::Debug for ReadOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadOptions")
            .field("credential", &self.credential)
            .field("profile", &self.profile)
            .field("plan", &self.plan)
            .field("bot_token", &self.bot_token.as_ref().map(|_| "[REDACTED]"))
            .field("archive_root", &self.archive_root)
            .field("read_policy", &self.read_policy)
            .finish()
    }
}
