use arbitrary::Arbitrary;
use facet::Facet;
use figue as args;

/// Manage the capture daemon for one state directory.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct WatchDaemonArgs {
    /// The local daemon operation.
    #[facet(args::subcommand)]
    pub command: WatchDaemonCommand,
}

use super::WatchDaemonCommand;

impl WatchDaemonArgs {
    /// # Errors
    /// Propagates request validation, operation and output failures.
    pub async fn invoke(self) -> eyre::Result<crate::cli::output::CliOutput> {
        match self.command {
            WatchDaemonCommand::Serve(args) => args.invoke().await,
            WatchDaemonCommand::Status(args) => args.invoke().await,
            WatchDaemonCommand::Stop(args) => args.invoke().await,
        }
    }
}
