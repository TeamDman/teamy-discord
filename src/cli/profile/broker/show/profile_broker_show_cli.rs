use arbitrary::Arbitrary;
use facet::Facet;
use figue as args;
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct ProfileBrokerShowArgs {
    /// Existing profile name; omit to use the active profile.
    #[facet(args::positional)]
    pub name: Option<String>,
}
impl ProfileBrokerShowArgs {
    /// # Errors
    /// Returns a safe error if there is no selected profile or the file is invalid.
    pub fn invoke(
        self,
        home: &crate::paths::AppHome,
    ) -> eyre::Result<crate::cli::output::CliOutput> {
        let threshold = Some(std::time::Duration::from_secs(1));
        self.invoke_work(home)?;
        Ok(crate::cli::output::CliOutput::none(threshold))
    }

    fn invoke_work(self, home: &crate::paths::AppHome) -> eyre::Result<()> {
        let profiles = crate::paths::Profiles::load(home)?;
        let profile = profiles
            .selected(self.name.as_deref())?
            .ok_or_else(|| eyre::eyre!("Select a profile with profile use <name>"))?;
        crate::json_stdout::print_facet_json(&profile.broker)
    }
}
