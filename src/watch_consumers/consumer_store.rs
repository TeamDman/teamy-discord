use super::ConsumerAcknowledgment;
use super::ConsumerClaim;
use super::ConsumerClaimResult;
use super::ConsumerDelivery;
use super::ConsumerDeliveryId;
use super::ConsumerError;
use super::ConsumerEvent;
use super::ConsumerName;
use super::ConsumerProvenance;
use super::ConsumerPublishReport;
use super::ConsumerRegistration;
use super::ConsumerRequestBinding;
use super::ConsumerRequestId;
use super::ConsumerRequestRecord;
use super::ConsumerStart;
use super::ConsumerState;
use super::ConsumerStatus;
use crate::watcher::WatcherMessage;
use crate::watcher::WatcherMessageId;
use crate::watcher::WatcherScope;
use eyre::WrapErr;
use std::collections::BTreeSet;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

const SNAPSHOT_FILE: &str = "consumers-state.json";
const PENDING_FILE: &str = "consumers-state.json.pending";
const LOCK_FILE: &str = "consumers-state.lock";
const MAX_SNAPSHOT_BYTES: u64 = 64 * 1024 * 1024;

/// The daemon actor exclusively owns this handle. A permanent separate OS-lock
/// file protects the snapshot for its lifetime; process exit releases the lock.
/// All events, claims, acknowledgments and request tombstones are retained.
/// Capacity/persistence failure stops mutation instead of pruning history.
///
/// Commit writes and syncs a same-directory staging file before atomic rename,
/// never deleting the destination first. Stale staging is uncommitted and ignored.
/// Local cooperating writers are supported; network shares, noncooperating edits
/// and universal device/power-loss durability are outside this guarantee.
/// A dispatch response is returned only after claim persistence succeeds. There
/// is no claim timer, reassignment or external I/O in this store.
#[derive(Debug)]
pub struct ConsumerStore {
    directory: PathBuf,
    state: ConsumerState,
    _lock: File,
}

impl ConsumerStore {
    /// # Errors
    /// Rejects invalid scope, competing writers, corrupt snapshots and failed I/O.
    pub fn open(directory: impl AsRef<Path>, scope: WatcherScope) -> eyre::Result<Self> {
        scope.validate().wrap_err(ConsumerError::ScopeMismatch)?;
        let directory = directory.as_ref().to_owned();
        std::fs::create_dir_all(&directory).wrap_err(ConsumerError::Persistence)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join(LOCK_FILE))
            .wrap_err(ConsumerError::Persistence)?;
        lock.try_lock()
            .map_err(|_redacted_error| eyre::eyre!(ConsumerError::StateBusy))?;
        let snapshot = directory.join(SNAPSHOT_FILE);
        let (state, initialize) = match std::fs::metadata(&snapshot) {
            Ok(metadata) => {
                eyre::ensure!(
                    metadata.is_file() && metadata.len() <= MAX_SNAPSHOT_BYTES,
                    ConsumerError::CorruptState
                );
                let text =
                    std::fs::read_to_string(&snapshot).wrap_err(ConsumerError::CorruptState)?;
                let state: ConsumerState =
                    facet_json::from_str(&text).wrap_err(ConsumerError::CorruptState)?;
                state.validate().wrap_err(ConsumerError::CorruptState)?;
                eyre::ensure!(state.scope == scope, ConsumerError::ScopeMismatch);
                (state, false)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                (ConsumerState::new(scope)?, true)
            }
            Err(error) => return Err(error).wrap_err(ConsumerError::Persistence),
        };
        let mut store = Self {
            directory,
            state,
            _lock: lock,
        };
        if initialize {
            store.commit(store.state.clone())?;
        }
        Ok(store)
    }

    #[must_use]
    pub const fn state(&self) -> &ConsumerState {
        &self.state
    }

    /// Append one eligible recovered batch in ascending Discord ID order.
    /// Already retained IDs preserve their first captured payload and sequence.
    /// A later delayed event gets a new local sequence, even if its Discord ID is older.
    /// # Errors
    /// Invalid/ineligible events reject the entire batch before any commit.
    pub fn publish(
        &mut self,
        mut messages: Vec<WatcherMessage>,
    ) -> eyre::Result<ConsumerPublishReport> {
        for message in &messages {
            eyre::ensure!(
                message
                    .is_trigger(&self.state.scope)
                    .wrap_err(ConsumerError::ScopeMismatch)?,
                ConsumerError::ScopeMismatch
            );
        }
        messages.sort_by_key(|message| message.id);
        let mut next = self.state.clone();
        let mut known: BTreeSet<_> = next.events.iter().map(|event| event.message.id).collect();
        let mut appended = 0_u64;
        for message in messages {
            if known.insert(message.id) {
                let sequence = next
                    .last_sequence()
                    .checked_add(1)
                    .ok_or(ConsumerError::Persistence)?;
                next.events.push(ConsumerEvent { sequence, message });
                appended += 1;
            }
        }
        let last_sequence = next.last_sequence();
        if appended > 0 {
            self.commit(next)?;
        }
        Ok(ConsumerPublishReport {
            appended,
            last_sequence,
        })
    }

    /// Registration is explicit; an existing name always restores its cursor.
    /// The daemon must call this after its catch-up publication barrier.
    /// # Errors
    /// Rejects invalid names and persistence failure.
    pub fn create(
        &mut self,
        name: ConsumerName,
        start: ConsumerStart,
    ) -> eyre::Result<ConsumerStatus> {
        ConsumerName::new(name.as_str())?;
        if self
            .state
            .consumers
            .iter()
            .any(|consumer| consumer.name == name)
        {
            return self.inspect(&name);
        }
        let mut next = self.state.clone();
        let head = next.last_sequence();
        let cursor_sequence = if start == ConsumerStart::Now { head } else { 0 };
        let status = ConsumerStatus {
            name: name.clone(),
            cursor_sequence,
            in_flight: None,
            last_published_sequence: head,
        };
        next.consumers.push(ConsumerRegistration {
            name,
            start,
            created_sequence: head,
            cursor_sequence,
            in_flight: None,
        });
        self.commit(next)?;
        Ok(status)
    }

    /// Durable correlation binding for create/inspect/replay/ack IPC requests.
    /// Claim performs this binding itself in its claim transaction.
    /// # Errors
    /// Rejects request ID reuse with changed operation, consumer or parameters.
    pub fn bind_request(
        &mut self,
        id: ConsumerRequestId,
        binding: ConsumerRequestBinding,
    ) -> eyre::Result<()> {
        let mut next = self.state.clone();
        let before = next.requests.len();
        Self::ensure_request(&mut next, id, binding)?;
        if next.requests.len() != before {
            self.commit(next)?;
        }
        Ok(())
    }

    /// Claim one next sequence. First delivery returns payload after commit;
    /// every uncertain retry returns metadata only, until explicit replay or ack.
    /// # Errors
    /// Rejects unknown consumers, conflicting request IDs and failed persistence.
    pub fn claim(
        &mut self,
        name: &ConsumerName,
        request_id: ConsumerRequestId,
    ) -> eyre::Result<ConsumerClaimResult> {
        self.inspect(name)?;
        let mut next = self.state.clone();
        let request_index = Self::ensure_request(
            &mut next,
            request_id.clone(),
            ConsumerRequestBinding::claim(name.clone()),
        )?;
        // Retained completion must win over an unrelated newer in-flight claim.
        if let Some(ack) = &next.requests[request_index].completed_ack {
            return Ok(ConsumerClaimResult::AlreadyCompleted(ack.clone()));
        }
        let consumer_index = next
            .consumers
            .iter()
            .position(|consumer| &consumer.name == name)
            .ok_or(ConsumerError::UnknownConsumer)?;
        if let Some(claim) = next.consumers[consumer_index].in_flight.clone() {
            next.requests[request_index].claim = Some(claim.clone());
            self.commit(next)?;
            return Ok(ConsumerClaimResult::PendingAck(claim));
        }
        let cursor = next.consumers[consumer_index].cursor_sequence;
        let Some(event) = next
            .events
            .iter()
            .find(|event| event.sequence > cursor)
            .cloned()
        else {
            // Retain the operation binding while the daemon owns the active waiter.
            self.commit(next)?;
            return Ok(ConsumerClaimResult::Empty);
        };
        let claim = ConsumerClaim {
            consumer: name.clone(),
            event_id: event.message.id,
            event_sequence: event.sequence,
            delivery_id: ConsumerDeliveryId::for_event(&next.scope, name, event.sequence),
            request_id,
        };
        next.consumers[consumer_index].in_flight = Some(claim.clone());
        next.requests[request_index].claim = Some(claim.clone());
        self.commit(next)?;
        Ok(ConsumerClaimResult::Claimed(ConsumerDelivery {
            claim,
            message: event.message,
        }))
    }

    /// # Errors
    /// Rejects invalid or unknown consumer names.
    pub fn inspect(&self, name: &ConsumerName) -> eyre::Result<ConsumerStatus> {
        ConsumerName::new(name.as_str())?;
        let consumer = self
            .state
            .consumers
            .iter()
            .find(|consumer| &consumer.name == name)
            .ok_or(ConsumerError::UnknownConsumer)?;
        Ok(ConsumerStatus {
            name: name.clone(),
            cursor_sequence: consumer.cursor_sequence,
            in_flight: consumer.in_flight.clone(),
            last_published_sequence: self.state.last_sequence(),
        })
    }

    /// Explicitly replay only the exact current delivery, preserving its identity.
    /// # Errors
    /// Rejects absent, completed, wrong-consumer or mismatched delivery IDs.
    pub fn replay(
        &self,
        name: &ConsumerName,
        delivery_id: &ConsumerDeliveryId,
    ) -> eyre::Result<ConsumerDelivery> {
        ConsumerDeliveryId::new(delivery_id.as_str())?;
        let claim = self
            .inspect(name)?
            .in_flight
            .ok_or(ConsumerError::InvalidDelivery)?;
        eyre::ensure!(
            &claim.delivery_id == delivery_id,
            ConsumerError::InvalidDelivery
        );
        let event = self
            .state
            .event(claim.event_sequence)
            .ok_or(ConsumerError::CorruptState)?;
        Ok(ConsumerDelivery {
            claim,
            message: event.message.clone(),
        })
    }

    /// Persist completion and advance only this consumer. Identical ack retries
    /// succeed even after another event has been claimed by the same consumer.
    /// # Errors
    /// Rejects mismatched event/delivery IDs, conflicting receipts and failed I/O.
    pub fn ack(
        &mut self,
        name: &ConsumerName,
        event_id: WatcherMessageId,
        delivery_id: &ConsumerDeliveryId,
        receipt: String,
    ) -> eyre::Result<ConsumerAcknowledgment> {
        ConsumerRequestBinding::validate_receipt(&receipt)?;
        WatcherMessageId::new(event_id.get())?;
        ConsumerDeliveryId::new(delivery_id.as_str())?;
        self.inspect(name)?;
        if let Some(ack) = self.state.acknowledgments.iter().find(|ack| {
            &ack.consumer == name && (ack.event_id == event_id || &ack.delivery_id == delivery_id)
        }) {
            eyre::ensure!(
                ack.event_id == event_id && &ack.delivery_id == delivery_id,
                ConsumerError::InvalidDelivery
            );
            eyre::ensure!(ack.receipt == receipt, ConsumerError::InvalidReceipt);
            return Ok(ack.clone());
        }
        let mut next = self.state.clone();
        let consumer = next
            .consumers
            .iter_mut()
            .find(|consumer| &consumer.name == name)
            .ok_or(ConsumerError::UnknownConsumer)?;
        let claim = consumer
            .in_flight
            .as_ref()
            .ok_or(ConsumerError::InvalidDelivery)?;
        eyre::ensure!(
            claim.event_id == event_id && &claim.delivery_id == delivery_id,
            ConsumerError::InvalidDelivery
        );
        let ack = ConsumerAcknowledgment {
            consumer: name.clone(),
            event_id,
            event_sequence: claim.event_sequence,
            delivery_id: delivery_id.clone(),
            receipt,
        };
        consumer.cursor_sequence = claim.event_sequence;
        consumer.in_flight = None;
        for request in &mut next.requests {
            if request
                .claim
                .as_ref()
                .is_some_and(|claim| &claim.consumer == name && &claim.delivery_id == delivery_id)
            {
                request.completed_ack = Some(ack.clone());
            }
        }
        next.acknowledgments.push(ack.clone());
        self.commit(next)?;
        Ok(ack)
    }

    fn ensure_request(
        state: &mut ConsumerState,
        id: ConsumerRequestId,
        binding: ConsumerRequestBinding,
    ) -> eyre::Result<usize> {
        eyre::ensure!(
            ConsumerRequestId::new(id.as_str())? == id,
            ConsumerError::InvalidIdentifier
        );
        binding.validate()?;
        if let Some(index) = state.requests.iter().position(|request| request.id == id) {
            eyre::ensure!(
                state.requests[index].binding == binding,
                ConsumerError::RequestConflict
            );
            return Ok(index);
        }
        let index = state.requests.len();
        state.requests.push(ConsumerRequestRecord {
            id,
            binding,
            claim: None,
            completed_ack: None,
        });
        Ok(index)
    }

    fn commit(&mut self, mut next: ConsumerState) -> eyre::Result<()> {
        next.provenance.updated_unix_ms =
            ConsumerProvenance::now().wrap_err(ConsumerError::Persistence)?;
        next.validate().wrap_err(ConsumerError::CorruptState)?;
        let encoded = facet_json::to_string_pretty(&next).wrap_err(ConsumerError::Persistence)?;
        eyre::ensure!(
            encoded.len() as u64 <= MAX_SNAPSHOT_BYTES,
            ConsumerError::Persistence
        );
        let pending = self.directory.join(PENDING_FILE);
        let mut staged = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&pending)
            .wrap_err(ConsumerError::Persistence)?;
        staged
            .write_all(encoded.as_bytes())
            .wrap_err(ConsumerError::Persistence)?;
        staged.sync_all().wrap_err(ConsumerError::Persistence)?;
        drop(staged);
        std::fs::rename(pending, self.directory.join(SNAPSHOT_FILE))
            .wrap_err(ConsumerError::Persistence)?;
        self.state = next;
        #[cfg(unix)]
        File::open(&self.directory)
            .and_then(|directory| directory.sync_all())
            .wrap_err(ConsumerError::Persistence)?;
        Ok(())
    }
}
