use crate::cli::thread::list::ThreadListArgs;
use crate::discord::read_context::ReadContext;
use crate::discord::read_context::ReadOptions;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
use figue::{self as args};

/// Live thread queries.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct ThreadArgs {
    #[facet(flatten)]
    pub options: ReadOptions,
    /// The thread subcommand to run.
    #[facet(args::subcommand)]
    pub command: ThreadCommand,
}

/// Thread subcommands.
// cli[impl command.surface.live-thread]
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum ThreadCommand {
    /// List active threads in a guild.
    List(ThreadListArgs),
}

impl ThreadArgs {
    /// # Errors
    ///
    /// This function will return an error if the subcommand fails.
    pub async fn invoke(self) -> Result<crate::cli::output::CliOutput> {
        let context = ReadContext::load(self.options)?;
        match self.command {
            ThreadCommand::List(args) => args.invoke(&context).await,
        }
    }
}
