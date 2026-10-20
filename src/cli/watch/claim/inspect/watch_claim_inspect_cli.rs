use arbitrary::Arbitrary;
use facet::Facet;
use figue as args;

/// Read claim metadata without replaying the captured Discord message.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct WatchClaimInspectArgs {
    #[facet(flatten)]
    pub options: crate::cli::watch::WatchClientOptions,
    /// Named consumer whose independent cursor or claim is selected.
    #[facet(args::named)]
    pub consumer: String,
}

impl WatchClaimInspectArgs {
    /// # Errors
    /// Propagates invalid identifiers, IPC, output and unsuccessful outcome errors.
    pub async fn invoke(self) -> eyre::Result<crate::cli::output::CliOutput> {
        let command = crate::watch_daemon::DaemonCommand::Inspect(
            crate::watch_consumers::ConsumerName::new(self.consumer)?,
        );
        self.options.invoke(command).await
    }
}
