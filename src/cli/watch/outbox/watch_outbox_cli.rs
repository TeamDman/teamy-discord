use arbitrary::Arbitrary;
use facet::Facet;
use figue as args;

/// Acknowledge externally handled deliveries through local IPC.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct WatchOutboxArgs {
    /// The local outbox operation.
    #[facet(args::subcommand)]
    pub command: WatchOutboxCommand,
}

use super::WatchOutboxCommand;

impl WatchOutboxArgs {
    /// # Errors
    /// Propagates request validation, operation and output failures.
    pub async fn invoke(self) -> eyre::Result<crate::cli::output::CliOutput> {
        match self.command {
            WatchOutboxCommand::Ack(args) => args.invoke().await,
        }
    }
}
