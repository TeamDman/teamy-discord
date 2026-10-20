use crate::cli::channel::list::ChannelListArgs;
use crate::discord::read_context::ReadContext;
use crate::discord::read_context::ReadOptions;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
use figue::{self as args};

/// Live channel queries.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct ChannelArgs {
    #[facet(flatten)]
    pub options: ReadOptions,
    /// The channel subcommand to run.
    #[facet(args::subcommand)]
    pub command: ChannelCommand,
}

/// Channel subcommands.
// cli[impl command.surface.live-channel]
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum ChannelCommand {
    /// List channels in a guild.
    List(ChannelListArgs),
}

impl ChannelArgs {
    /// # Errors
    ///
    /// This function will return an error if the subcommand fails.
    pub async fn invoke(self) -> Result<crate::cli::output::CliOutput> {
        let context = ReadContext::load(self.options)?;
        match self.command {
            ChannelCommand::List(args) => args.invoke(&context).await,
        }
    }
}
