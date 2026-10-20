use crate::cli::message::list::MessageListArgs;
use crate::discord::read_context::ReadContext;
use crate::discord::read_context::ReadOptions;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
use figue::{self as args};

/// Live message queries.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct MessageArgs {
    #[facet(flatten)]
    pub options: ReadOptions,
    /// The message subcommand to run.
    #[facet(args::subcommand)]
    pub command: MessageCommand,
}

/// Message subcommands.
// cli[impl command.surface.live-message]
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum MessageCommand {
    /// List messages in a channel or thread.
    List(MessageListArgs),
}

impl MessageArgs {
    /// # Errors
    ///
    /// This function will return an error if the subcommand fails.
    pub async fn invoke(self) -> Result<crate::cli::output::CliOutput> {
        let context = ReadContext::load(self.options)?;
        match self.command {
            MessageCommand::List(args) => args.invoke(&context).await,
        }
    }
}
