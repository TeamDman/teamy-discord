use arbitrary::Arbitrary;
use facet::Facet;
use figue as args;

/// Acknowledge one exact delivery after external handling completes. This command sends no Discord or GitHub message.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct WatchOutboxAckArgs {
    #[facet(flatten)]
    pub options: crate::cli::watch::WatchClientOptions,
    /// Named consumer whose independent cursor or claim is selected.
    #[facet(args::named)]
    pub consumer: String,
    /// Discord message ID identifying the captured event being acknowledged.
    #[facet(args::named)]
    pub event_id: u64,
    /// Exact d1- delivery identity previously returned for this consumer.
    #[facet(args::named)]
    pub delivery_id: String,
    /// Nonempty handling receipt, at most 4096 bytes and without control characters.
    #[facet(args::named)]
    pub receipt: String,
}

impl WatchOutboxAckArgs {
    /// # Errors
    /// Propagates invalid identifiers, IPC, output and unsuccessful outcome errors.
    pub async fn invoke(self) -> eyre::Result<crate::cli::output::CliOutput> {
        let command = crate::watch_daemon::DaemonCommand::Ack {
            consumer: crate::watch_consumers::ConsumerName::new(self.consumer)?,
            event_id: crate::watcher::WatcherMessageId::new(self.event_id)?,
            delivery_id: crate::watch_consumers::ConsumerDeliveryId::new(self.delivery_id)?,
            receipt: self.receipt,
        };
        self.options.invoke(command).await
    }
}
