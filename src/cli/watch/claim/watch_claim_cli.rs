use arbitrary::Arbitrary;
use facet::Facet;
use figue as args;

/// Inspect or explicitly replay a pending delivery.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct WatchClaimArgs {
    /// The local claim operation.
    #[facet(args::subcommand)]
    pub command: WatchClaimCommand,
}

use super::WatchClaimCommand;

impl WatchClaimArgs {
    /// # Errors
    /// Propagates request validation, operation and output failures.
    pub async fn invoke(self) -> eyre::Result<crate::cli::output::CliOutput> {
        match self.command {
            WatchClaimCommand::Inspect(args) => args.invoke().await,
            WatchClaimCommand::Replay(args) => args.invoke().await,
        }
    }
}
