use arbitrary::Arbitrary;
use facet::Facet;
use figue as args;

/// Capture approved Discord events once and consume them through a local acknowledged outbox.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct WatchArgs {
    /// Watcher daemon, consumer, claim and acknowledgment operations.
    #[facet(args::subcommand)]
    pub command: super::WatchCommand,
}

impl WatchArgs {
    /// # Errors
    /// Propagates invalid requests, IPC failures, cancellation and unsuccessful daemon outcomes.
    pub async fn invoke(self) -> eyre::Result<crate::cli::output::CliOutput> {
        match self.command {
            super::WatchCommand::Daemon(args) => args.invoke().await,
            super::WatchCommand::Next(args) => args.invoke().await,
            super::WatchCommand::Consumer(args) => args.invoke().await,
            super::WatchCommand::Claim(args) => args.invoke().await,
            super::WatchCommand::Outbox(args) => args.invoke().await,
        }
    }
}
