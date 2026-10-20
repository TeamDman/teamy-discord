use arbitrary::Arbitrary;
use facet::Facet;

/// Unset the active profile while retaining all named profiles.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct ProfileClearArgs;
impl ProfileClearArgs {
    /// # Errors
    /// Returns an error for a competing update, invalid configuration or failed I/O.
    pub fn invoke(
        self,
        home: &crate::paths::AppHome,
    ) -> eyre::Result<crate::cli::output::CliOutput> {
        let threshold = Some(std::time::Duration::from_secs(1));
        Self::invoke_work(home)?;
        Ok(crate::cli::output::CliOutput::none(threshold))
    }

    fn invoke_work(home: &crate::paths::AppHome) -> eyre::Result<()> {
        let profiles = crate::paths::Profiles::update(home, |profiles| {
            profiles.clear_active();
            Ok(())
        })?;
        crate::json_stdout::print_facet_json(&profiles)
    }
}
