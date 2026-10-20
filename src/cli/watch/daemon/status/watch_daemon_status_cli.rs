use arbitrary::Arbitrary;
use facet::Facet;

/// Read local daemon status without acquiring credentials.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct WatchDaemonStatusArgs {
    #[facet(flatten)]
    pub options: crate::cli::watch::WatchClientOptions,
}

impl WatchDaemonStatusArgs {
    /// # Errors
    /// Propagates invalid identifiers, IPC, output and unsuccessful outcome errors.
    pub async fn invoke(self) -> eyre::Result<crate::cli::output::CliOutput> {
        let command = crate::watch_daemon::DaemonCommand::Status;
        self.options.invoke(command).await
    }
}
