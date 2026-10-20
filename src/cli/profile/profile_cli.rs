use super::profile_command::ProfileCommand;
use arbitrary::Arbitrary;
use facet::Facet;
use figue::{self as args};

/// Manage nonsecret local guild/channel and broker-reference defaults. Profiles do not grant access.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct ProfileArgs {
    #[facet(args::subcommand)]
    pub command: ProfileCommand,
}
impl ProfileArgs {
    /// # Errors
    /// Returns an error for invalid configuration, rejected scope or a failed operation.
    pub fn invoke(self) -> eyre::Result<crate::cli::output::CliOutput> {
        let home = &crate::paths::APP_HOME;
        match self.command {
            ProfileCommand::Broker(args) => args.invoke(home),
            ProfileCommand::Clear(args) => args.invoke(home),
            ProfileCommand::List(args) => args.invoke(home),
            ProfileCommand::Set(args) => args.invoke(home),
            ProfileCommand::Show(args) => args.invoke(home),
            ProfileCommand::UseProfile(args) => args.invoke(home),
        }
    }
}
