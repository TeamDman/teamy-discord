use crate::cli::user::list::UserListArgs;
use crate::discord::read_context::ReadContext;
use crate::discord::read_context::ReadOptions;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
use figue::{self as args};

/// Live user queries.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct UserArgs {
    #[facet(flatten)]
    pub options: ReadOptions,
    /// The user subcommand to run.
    #[facet(args::subcommand)]
    pub command: UserCommand,
}

/// User subcommands.
// cli[impl command.surface.live-user]
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum UserCommand {
    /// List users in a guild.
    List(UserListArgs),
}

impl UserArgs {
    /// # Errors
    ///
    /// This function will return an error if the subcommand fails.
    pub async fn invoke(self) -> Result<crate::cli::output::CliOutput> {
        let context = ReadContext::load(self.options)?;
        match self.command {
            UserCommand::List(args) => args.invoke(&context).await,
        }
    }
}
