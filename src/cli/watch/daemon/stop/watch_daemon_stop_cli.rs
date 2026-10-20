use arbitrary::Arbitrary;
use facet::Facet;

/// Request graceful daemon shutdown while retaining durable events and consumer claims.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct WatchDaemonStopArgs {
    #[facet(flatten)]
    pub options: crate::cli::watch::WatchClientOptions,
}

impl WatchDaemonStopArgs {
    /// # Errors
    /// Propagates invalid identifiers, IPC, output and unsuccessful outcome errors.
    pub async fn invoke(self) -> eyre::Result<crate::cli::output::CliOutput> {
        let command = crate::watch_daemon::DaemonCommand::Stop;
        self.options.invoke(command).await
    }
}
