use super::ConsumerAcknowledgment;
use super::ConsumerClaim;
use super::ConsumerDeliveryId;
use super::ConsumerError;
use super::ConsumerEvent;
use super::ConsumerName;
use super::ConsumerOperation;
use super::ConsumerProvenance;
use super::ConsumerRegistration;
use super::ConsumerRequestBinding;
use super::ConsumerRequestId;
use super::ConsumerRequestRecord;
use super::ConsumerStart;
use crate::watcher::WatcherScope;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, PartialEq, facet::Facet)]
pub struct ConsumerState {
    pub schema_version: u32,
    pub scope: WatcherScope,
    pub events: Vec<ConsumerEvent>,
    pub consumers: Vec<ConsumerRegistration>,
    pub requests: Vec<ConsumerRequestRecord>,
    pub acknowledgments: Vec<ConsumerAcknowledgment>,
    pub provenance: ConsumerProvenance,
}

impl ConsumerState {
    pub(super) fn new(scope: WatcherScope) -> eyre::Result<Self> {
        scope.validate()?;
        Ok(Self {
            schema_version: 1,
            scope,
            events: Vec::new(),
            consumers: Vec::new(),
            requests: Vec::new(),
            acknowledgments: Vec::new(),
            provenance: ConsumerProvenance::new()?,
        })
    }

    #[must_use]
    pub fn last_sequence(&self) -> u64 {
        self.events.last().map_or(0, |event| event.sequence)
    }

    pub(super) fn event(&self, sequence: u64) -> Option<&ConsumerEvent> {
        sequence
            .checked_sub(1)
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| self.events.get(index))
    }

    pub(super) fn validate_claim(&self, claim: &ConsumerClaim) -> eyre::Result<()> {
        ConsumerName::new(claim.consumer.as_str())?;
        eyre::ensure!(
            ConsumerRequestId::new(claim.request_id.as_str())? == claim.request_id,
            ConsumerError::CorruptState
        );
        ConsumerDeliveryId::new(claim.delivery_id.as_str())?;
        let event = self
            .event(claim.event_sequence)
            .ok_or(ConsumerError::CorruptState)?;
        eyre::ensure!(
            event.message.id == claim.event_id
                && claim.delivery_id
                    == ConsumerDeliveryId::for_event(
                        &self.scope,
                        &claim.consumer,
                        claim.event_sequence
                    ),
            ConsumerError::CorruptState
        );
        Ok(())
    }

    /// Validate every retained relationship before accepting a snapshot or commit.
    /// # Errors
    /// Rejects malformed IDs, altered scope, skipped cursors and inconsistent claims/tombstones.
    #[expect(
        clippy::too_many_lines,
        reason = "Validate all persisted relationships before accepting a snapshot"
    )]
    pub fn validate(&self) -> eyre::Result<()> {
        eyre::ensure!(self.schema_version == 1, ConsumerError::CorruptState);
        self.scope.validate()?;
        let mut message_ids = BTreeSet::new();
        for (index, event) in self.events.iter().enumerate() {
            eyre::ensure!(
                event.sequence
                    == u64::try_from(index)?
                        .checked_add(1)
                        .ok_or(ConsumerError::CorruptState)?
                    && message_ids.insert(event.message.id)
                    && event.message.is_trigger(&self.scope)?,
                ConsumerError::CorruptState
            );
        }
        let head = self.last_sequence();
        let mut names = BTreeSet::new();
        for consumer in &self.consumers {
            ConsumerName::new(consumer.name.as_str())?;
            eyre::ensure!(
                names.insert(consumer.name.clone())
                    && consumer.created_sequence <= head
                    && consumer.cursor_sequence <= head,
                ConsumerError::CorruptState
            );
            let initial = if consumer.start == ConsumerStart::Now {
                consumer.created_sequence
            } else {
                0
            };
            eyre::ensure!(
                consumer.cursor_sequence >= initial,
                ConsumerError::CorruptState
            );
            let mut acknowledged: Vec<_> = self
                .acknowledgments
                .iter()
                .filter(|ack| ack.consumer == consumer.name)
                .map(|ack| ack.event_sequence)
                .collect();
            acknowledged.sort_unstable();
            eyre::ensure!(
                u64::try_from(acknowledged.len())? == consumer.cursor_sequence - initial,
                ConsumerError::CorruptState
            );
            for (index, sequence) in acknowledged.iter().enumerate() {
                eyre::ensure!(
                    *sequence == initial + u64::try_from(index)? + 1,
                    ConsumerError::CorruptState
                );
            }
            if let Some(claim) = &consumer.in_flight {
                self.validate_claim(claim)?;
                eyre::ensure!(
                    claim.consumer == consumer.name
                        && claim.event_sequence
                            == consumer
                                .cursor_sequence
                                .checked_add(1)
                                .ok_or(ConsumerError::CorruptState)?,
                    ConsumerError::CorruptState
                );
                eyre::ensure!(
                    self.requests
                        .iter()
                        .any(|request| request.id == claim.request_id
                            && request.claim.as_ref() == Some(claim)
                            && request.completed_ack.is_none()),
                    ConsumerError::CorruptState
                );
            }
        }
        let mut deliveries = BTreeSet::new();
        for ack in &self.acknowledgments {
            ConsumerName::new(ack.consumer.as_str())?;
            ConsumerDeliveryId::new(ack.delivery_id.as_str())?;
            ConsumerRequestBinding::validate_receipt(&ack.receipt)?;
            let consumer = self
                .consumers
                .iter()
                .find(|consumer| consumer.name == ack.consumer)
                .ok_or(ConsumerError::CorruptState)?;
            let event = self
                .event(ack.event_sequence)
                .ok_or(ConsumerError::CorruptState)?;
            eyre::ensure!(
                ack.event_sequence <= consumer.cursor_sequence
                    && event.message.id == ack.event_id
                    && deliveries.insert(ack.delivery_id.clone())
                    && ack.delivery_id
                        == ConsumerDeliveryId::for_event(
                            &self.scope,
                            &ack.consumer,
                            ack.event_sequence
                        ),
                ConsumerError::CorruptState
            );
            eyre::ensure!(
                self.requests
                    .iter()
                    .any(|request| request.completed_ack.as_ref() == Some(ack)),
                ConsumerError::CorruptState
            );
        }
        let mut request_ids = BTreeSet::new();
        for request in &self.requests {
            eyre::ensure!(
                ConsumerRequestId::new(request.id.as_str())? == request.id
                    && request_ids.insert(request.id.clone()),
                ConsumerError::CorruptState
            );
            request.binding.validate()?;
            if let Some(claim) = &request.claim {
                self.validate_claim(claim)?;
                eyre::ensure!(
                    request.binding.operation == ConsumerOperation::Claim
                        && request.binding.consumer == claim.consumer,
                    ConsumerError::CorruptState
                );
                if let Some(ack) = &request.completed_ack {
                    eyre::ensure!(
                        ack.consumer == claim.consumer
                            && ack.event_id == claim.event_id
                            && ack.event_sequence == claim.event_sequence
                            && ack.delivery_id == claim.delivery_id
                            && self.acknowledgments.contains(ack),
                        ConsumerError::CorruptState
                    );
                } else {
                    eyre::ensure!(
                        self.consumers
                            .iter()
                            .any(|consumer| consumer.name == claim.consumer
                                && consumer.in_flight.as_ref() == Some(claim)),
                        ConsumerError::CorruptState
                    );
                }
            } else {
                eyre::ensure!(request.completed_ack.is_none(), ConsumerError::CorruptState);
            }
        }
        // Completed requests and pending aliases must retain the original claim
        // owner. Losing that anchor would let an old request ID allocate new work.
        // Resolve only after all request records have been structurally validated.
        let request_origins: BTreeMap<_, _> = self
            .requests
            .iter()
            .map(|request| (request.id.clone(), request))
            .collect();
        for request in &self.requests {
            if let Some(claim) = &request.claim {
                let origin = request_origins
                    .get(&claim.request_id)
                    .ok_or(ConsumerError::CorruptState)?;
                eyre::ensure!(
                    origin.claim.as_ref() == Some(claim)
                        && origin.binding == ConsumerRequestBinding::claim(claim.consumer.clone())
                        && origin.completed_ack == request.completed_ack,
                    ConsumerError::CorruptState
                );
            }
        }
        eyre::ensure!(
            !self.provenance.tool.is_empty()
                && !self.provenance.tool_version.is_empty()
                && !self.provenance.source_repository.is_empty()
                && !self.provenance.source_revision.is_empty(),
            ConsumerError::CorruptState
        );
        Ok(())
    }
}
