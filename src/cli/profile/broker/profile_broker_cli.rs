use arbitrary::Arbitrary;
use facet::Facet;
use figue as args;
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct ProfileBrokerArgs {
    #[facet(args::subcommand)]
    pub command: super::profile_broker_command::ProfileBrokerCommand,
}
impl ProfileBrokerArgs {
    /// # Errors
    /// Returns safe validation or local profile I/O errors; never contacts the broker or vault.
    pub fn invoke(
        self,
        home: &crate::paths::AppHome,
    ) -> eyre::Result<crate::cli::output::CliOutput> {
        use super::profile_broker_command::ProfileBrokerCommand;
        match self.command {
            ProfileBrokerCommand::Set(args) => args.invoke(home),
            ProfileBrokerCommand::Show(args) => args.invoke(home),
            ProfileBrokerCommand::Clear(args) => args.invoke(home),
        }
    }
}
