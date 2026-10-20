use crate::cli::guild::list::GuildListArgs;
use crate::discord::read_context::ReadContext;
use crate::discord::read_context::ReadOptions;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
use figue::{self as args};

/// Live guild queries.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct GuildArgs {
    #[facet(flatten)]
    pub options: ReadOptions,
    /// The guild subcommand to run.
    #[facet(args::subcommand)]
    pub command: GuildCommand,
}

/// Guild subcommands.
// cli[impl command.surface.live-guild]
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum GuildCommand {
    /// List guilds visible to the bot.
    List(GuildListArgs),
}

impl GuildArgs {
    /// # Errors
    ///
    /// This function will return an error if the subcommand fails.
    pub async fn invoke(self) -> Result<crate::cli::output::CliOutput> {
        let context = ReadContext::load(self.options)?;
        match self.command {
            GuildCommand::List(args) => args.invoke(&context).await,
        }
    }
}
