use arbitrary::Arbitrary;
use facet::Facet;
use figue as args;

/// Manage named consumers independently of Discord capture.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct WatchConsumerArgs {
    /// The local consumer operation.
    #[facet(args::subcommand)]
    pub command: WatchConsumerCommand,
}

use super::WatchConsumerCommand;

impl WatchConsumerArgs {
    /// # Errors
    /// Propagates request validation, operation and output failures.
    pub async fn invoke(self) -> eyre::Result<crate::cli::output::CliOutput> {
        match self.command {
            WatchConsumerCommand::Create(args) => args.invoke().await,
        }
    }
}
