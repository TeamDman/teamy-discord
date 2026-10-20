use arbitrary::Arbitrary;
use facet::Facet;
use figue as args;

/// Create a named consumer at now or oldest. Reusing a name preserves its existing durable cursor.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct WatchConsumerCreateArgs {
    #[facet(flatten)]
    pub options: crate::cli::watch::WatchClientOptions,
    /// Named consumer whose independent cursor or claim is selected.
    #[facet(args::named)]
    pub consumer: String,
    /// Start after current events (now), or at the oldest retained event (oldest).
    #[facet(args::named)]
    pub from: super::WatchConsumerStartCli,
}

impl WatchConsumerCreateArgs {
    /// # Errors
    /// Propagates invalid identifiers, IPC, output and unsuccessful outcome errors.
    pub async fn invoke(self) -> eyre::Result<crate::cli::output::CliOutput> {
        let command = crate::watch_daemon::DaemonCommand::CreateConsumer {
            consumer: crate::watch_consumers::ConsumerName::new(self.consumer)?,
            start: self.from.into(),
        };
        self.options.invoke(command).await
    }
}
