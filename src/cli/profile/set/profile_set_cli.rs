use arbitrary::Arbitrary;
use facet::Facet;
use figue::{self as args};
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct ProfileSetArgs {
    /// Local profile name.
    #[facet(args::positional)]
    pub name: String,
    /// Default guild. Nonzero Discord ID.
    #[facet(args::named)]
    pub guild_id: u64,
    /// Optional default channel. Saving without this clears the previous default.
    #[facet(args::named)]
    pub channel_id: Option<u64>,
}
impl ProfileSetArgs {
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
        let profiles = crate::paths::Profiles::update(home, |profiles| {
            profiles.set(crate::paths::Profile {
                name: self.name,
                guild_id: self.guild_id,
                channel_id: self.channel_id,
                broker: None,
            })
        })?;
        crate::json_stdout::print_facet_json(&profiles)
    }
}
