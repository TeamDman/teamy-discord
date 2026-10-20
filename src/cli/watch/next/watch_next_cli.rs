use arbitrary::Arbitrary;
use facet::Facet;
use figue as args;

/// Wait locally for one delivery. JSON stdout is one result; a pending acknowledgment never auto-replays its payload.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct WatchNextArgs {
    #[facet(flatten)]
    pub options: crate::cli::watch::WatchClientOptions,
    /// Named consumer whose independent cursor or claim is selected.
    #[facet(args::named)]
    pub consumer: String,
}

impl WatchNextArgs {
    /// # Errors
    /// Propagates invalid identifiers, IPC, output and unsuccessful outcome errors.
    pub async fn invoke(self) -> eyre::Result<crate::cli::output::CliOutput> {
        let command = crate::watch_daemon::DaemonCommand::Next(
            crate::watch_consumers::ConsumerName::new(self.consumer)?,
        );
        self.options.invoke(command).await
    }
}
