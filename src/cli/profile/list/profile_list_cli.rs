use arbitrary::Arbitrary;
use facet::Facet;
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct ProfileListArgs;
impl ProfileListArgs {
    /// # Errors
    /// Returns an error for invalid configuration, rejected scope or a failed operation.
    pub fn invoke(
        self,
        home: &crate::paths::AppHome,
    ) -> eyre::Result<crate::cli::output::CliOutput> {
        let threshold = Some(std::time::Duration::from_secs(1));
        Self::invoke_work(home)?;
        Ok(crate::cli::output::CliOutput::none(threshold))
    }

    fn invoke_work(home: &crate::paths::AppHome) -> eyre::Result<()> {
        crate::json_stdout::print_facet_json(&crate::paths::Profiles::load(home)?)
    }
}
