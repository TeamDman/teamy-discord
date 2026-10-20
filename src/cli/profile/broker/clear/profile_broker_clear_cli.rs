use arbitrary::Arbitrary;
use facet::Facet;
use figue as args;
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct ProfileBrokerClearArgs {
    /// Existing profile name.
    #[facet(args::positional)]
    pub name: String,
    /// Clear only the account default.
    #[facet(args::named, default)]
    pub account: bool,
    /// Clear only the item-field reference default.
    #[facet(args::named, default)]
    pub secret_ref: bool,
    /// Clear both metadata fields; conflicts with individual choices.
    #[facet(args::named, default)]
    pub all: bool,
}
impl ProfileBrokerClearArgs {
    /// # Errors
    /// Requires an explicit clear selection and existing profile. Does not contact the broker.
    pub fn invoke(
        self,
        home: &crate::paths::AppHome,
    ) -> eyre::Result<crate::cli::output::CliOutput> {
        let threshold = Some(std::time::Duration::from_secs(1));
        self.invoke_work(home)?;
        Ok(crate::cli::output::CliOutput::none(threshold))
    }

    fn invoke_work(self, home: &crate::paths::AppHome) -> eyre::Result<()> {
        let result = crate::paths::BrokerReferenceChange::Clear {
            name: self.name,
            account: self.account,
            secret_ref: self.secret_ref,
            all: self.all,
        }
        .apply(home)?;
        crate::json_stdout::print_facet_json(&result)
    }
}
