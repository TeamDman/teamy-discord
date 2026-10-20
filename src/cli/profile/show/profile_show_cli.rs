use arbitrary::Arbitrary;
use facet::Facet;
use figue::{self as args};
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct ProfileShowArgs {
    /// Name to inspect; otherwise show the active profile.
    #[facet(args::positional)]
    pub name: Option<String>,
}
impl ProfileShowArgs {
    /// # Errors
    /// Returns an error for invalid configuration, rejected scope or a failed operation.
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
        crate::json_stdout::print_facet_json(&profiles.selected(self.name.as_deref())?)
    }
}
