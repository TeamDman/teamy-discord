use super::DaemonCommand;
use super::PROTOCOL_VERSION;
use crate::watch_consumers::ConsumerDeliveryId;
use crate::watch_consumers::ConsumerName;
use crate::watch_consumers::ConsumerRequestId;
#[derive(Clone, Debug, facet::Facet)]
pub struct DaemonRequest {
    pub version: u32,
    pub request_id: ConsumerRequestId,
    pub command: DaemonCommand,
}
impl DaemonRequest {
    /// # Errors
    /// Revalidates typed values deserialized at the untrusted IPC boundary.
    pub fn validate(&self) -> eyre::Result<()> {
        eyre::ensure!(
            self.version == PROTOCOL_VERSION,
            "Unsupported watcher IPC protocol"
        );
        eyre::ensure!(
            ConsumerRequestId::new(self.request_id.as_str())? == self.request_id,
            "Noncanonical watcher request identity"
        );
        match &self.command {
            DaemonCommand::Next(name) | DaemonCommand::Inspect(name) => {
                ConsumerName::new(name.as_str())?;
            }
            DaemonCommand::CreateConsumer { consumer, .. } => {
                ConsumerName::new(consumer.as_str())?;
            }
            DaemonCommand::Replay {
                consumer,
                delivery_id,
            } => {
                ConsumerName::new(consumer.as_str())?;
                eyre::ensure!(
                    ConsumerDeliveryId::new(delivery_id.as_str())? == *delivery_id,
                    "Noncanonical delivery identity"
                );
            }
            DaemonCommand::Ack {
                consumer,
                event_id,
                delivery_id,
                receipt,
            } => {
                ConsumerName::new(consumer.as_str())?;
                eyre::ensure!(
                    ConsumerDeliveryId::new(delivery_id.as_str())? == *delivery_id,
                    "Noncanonical delivery identity"
                );
                crate::watcher::WatcherMessageId::new(event_id.get())?;
                eyre::ensure!(
                    !receipt.trim().is_empty()
                        && receipt.len() <= 4096
                        && !receipt.chars().any(char::is_control),
                    "Invalid acknowledgment receipt"
                );
            }
            DaemonCommand::Status | DaemonCommand::Stop => {}
        }
        Ok(())
    }
}
