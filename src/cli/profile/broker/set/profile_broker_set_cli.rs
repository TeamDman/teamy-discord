use arbitrary::Arbitrary;
use facet::Facet;
use figue as args;
#[derive(Facet, Arbitrary, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct ProfileBrokerSetArgs {
    /// Existing local profile name.
    #[facet(args::positional)]
    pub name: String,
    /// 1Password account UUID, email or sign-in address; never a service credential.
    #[facet(args::named)]
    pub account: Option<String>,
    /// Exact `op://vault/item/field` address; never the field value.
    #[facet(args::named)]
    pub secret_ref: Option<String>,
}
impl std::fmt::Debug for ProfileBrokerSetArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ProfileBrokerSetArgs([nonsecret metadata redacted])")
    }
}
impl ProfileBrokerSetArgs {
    /// # Errors
    /// Requires at least one valid metadata field and an existing profile; preserves omitted fields.
    pub fn invoke(
        self,
        home: &crate::paths::AppHome,
    ) -> eyre::Result<crate::cli::output::CliOutput> {
        let threshold = Some(std::time::Duration::from_secs(1));
        self.invoke_work(home)?;
        Ok(crate::cli::output::CliOutput::none(threshold))
    }

    fn invoke_work(self, home: &crate::paths::AppHome) -> eyre::Result<()> {
        let result = crate::paths::BrokerReferenceChange::Set {
            name: self.name,
            account: self.account,
            secret_ref: self.secret_ref,
        }
        .apply(home)?;
        crate::json_stdout::print_facet_json(&result)
    }
}
