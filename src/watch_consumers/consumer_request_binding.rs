use super::ConsumerDeliveryId;
use super::ConsumerError;
use super::ConsumerName;
use super::ConsumerOperation;
use super::ConsumerStart;
use crate::watcher::WatcherMessageId;

/// A request ID cannot silently change its operation, consumer or parameters.
#[derive(Clone, Debug, Eq, PartialEq, facet::Facet)]
pub struct ConsumerRequestBinding {
    pub operation: ConsumerOperation,
    pub consumer: ConsumerName,
    pub start: Option<ConsumerStart>,
    pub event_id: Option<WatcherMessageId>,
    pub delivery_id: Option<ConsumerDeliveryId>,
    pub receipt: Option<String>,
}

impl ConsumerRequestBinding {
    #[must_use]
    pub fn create(consumer: ConsumerName, start: ConsumerStart) -> Self {
        Self {
            operation: ConsumerOperation::Create,
            consumer,
            start: Some(start),
            event_id: None,
            delivery_id: None,
            receipt: None,
        }
    }
    #[must_use]
    pub fn claim(consumer: ConsumerName) -> Self {
        Self {
            operation: ConsumerOperation::Claim,
            consumer,
            start: None,
            event_id: None,
            delivery_id: None,
            receipt: None,
        }
    }
    #[must_use]
    pub fn inspect(consumer: ConsumerName) -> Self {
        Self {
            operation: ConsumerOperation::Inspect,
            ..Self::claim(consumer)
        }
    }
    #[must_use]
    pub fn replay(consumer: ConsumerName, delivery_id: ConsumerDeliveryId) -> Self {
        Self {
            operation: ConsumerOperation::Replay,
            delivery_id: Some(delivery_id),
            ..Self::claim(consumer)
        }
    }
    #[must_use]
    pub fn ack(
        consumer: ConsumerName,
        event_id: WatcherMessageId,
        delivery_id: ConsumerDeliveryId,
        receipt: String,
    ) -> Self {
        Self {
            operation: ConsumerOperation::Acknowledge,
            event_id: Some(event_id),
            delivery_id: Some(delivery_id),
            receipt: Some(receipt),
            ..Self::claim(consumer)
        }
    }

    pub(super) fn validate(&self) -> eyre::Result<()> {
        ConsumerName::new(self.consumer.as_str())?;
        let valid = match self.operation {
            ConsumerOperation::Create => {
                self.start.is_some()
                    && self.event_id.is_none()
                    && self.delivery_id.is_none()
                    && self.receipt.is_none()
            }
            ConsumerOperation::Claim | ConsumerOperation::Inspect => {
                self.start.is_none()
                    && self.event_id.is_none()
                    && self.delivery_id.is_none()
                    && self.receipt.is_none()
            }
            ConsumerOperation::Replay => {
                self.start.is_none()
                    && self.event_id.is_none()
                    && self.delivery_id.is_some()
                    && self.receipt.is_none()
            }
            ConsumerOperation::Acknowledge => {
                self.start.is_none()
                    && self.event_id.is_some()
                    && self.delivery_id.is_some()
                    && self.receipt.is_some()
            }
        };
        eyre::ensure!(valid, ConsumerError::RequestConflict);
        if let Some(event_id) = self.event_id {
            WatcherMessageId::new(event_id.get())?;
        }
        if let Some(delivery_id) = &self.delivery_id {
            ConsumerDeliveryId::new(delivery_id.as_str())?;
        }
        if let Some(receipt) = &self.receipt {
            Self::validate_receipt(receipt)?;
        }
        Ok(())
    }

    pub(super) fn validate_receipt(receipt: &str) -> eyre::Result<()> {
        eyre::ensure!(
            !receipt.trim().is_empty()
                && receipt.len() <= 4096
                && !receipt.chars().any(char::is_control),
            ConsumerError::InvalidReceipt
        );
        Ok(())
    }
}
