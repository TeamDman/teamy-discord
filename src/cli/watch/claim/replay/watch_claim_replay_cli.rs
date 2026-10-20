use arbitrary::Arbitrary;
use facet::Facet;
use figue as args;

/// Explicitly replay the exact in-flight delivery. Replaying does not acknowledge handling.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct WatchClaimReplayArgs {
    #[facet(flatten)]
    pub options: crate::cli::watch::WatchClientOptions,
    /// Named consumer whose independent cursor or claim is selected.
    #[facet(args::named)]
    pub consumer: String,
    /// Exact d1- delivery identity previously returned for this consumer.
    #[facet(args::named)]
    pub delivery_id: String,
}

impl WatchClaimReplayArgs {
    /// # Errors
    /// Propagates invalid identifiers, IPC, output and unsuccessful outcome errors.
    pub async fn invoke(self) -> eyre::Result<crate::cli::output::CliOutput> {
        let command = crate::watch_daemon::DaemonCommand::Replay {
            consumer: crate::watch_consumers::ConsumerName::new(self.consumer)?,
            delivery_id: crate::watch_consumers::ConsumerDeliveryId::new(self.delivery_id)?,
        };
        self.options.invoke(command).await
    }
}
