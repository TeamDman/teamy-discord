use super::WatchOutboxEntry;
use super::WatchProvenance;
use super::WatchSweepReport;
use super::WatchSweepRequest;
use super::WatchSweepResume;
use super::WatcherError;
use super::WatcherMessage;
use super::WatcherMessageId;
use super::WatcherScope;
use super::WatcherSource;
use super::WatcherState;
use eyre::WrapErr;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

const SNAPSHOT_FILE: &str = "watcher-state.json";
const PENDING_FILE: &str = "watcher-state.json.pending";
const LOCK_FILE: &str = "watcher-state.lock";
const MAX_SNAPSHOT_BYTES: u64 = 64 * 1024 * 1024;

/// One exclusive OS lock protects a scoped snapshot for this handle's lifetime.
///
/// Drop between sweeps so local outbox list/ack commands can acquire the lock.
/// The permanent lock file is never deleted: OS handle closure releases the lock,
/// including process termination. A stale `.pending` is uncommitted and ignored;
/// a later commit overwrites it only while holding the same exclusive lock.
///
/// Snapshots use same-directory write + `sync_all` + atomic rename/replacement,
/// never delete-then-rename. Supported local filesystems see old or new snapshots
/// after a process crash. Windows/filesystem/device power-loss guarantees vary;
/// this is not a universal power-loss transaction. Corrupt committed snapshots
/// fail closed instead of being replaced with an empty state. Network shares and
/// noncooperating writers are outside the locking/atomicity guarantee.
#[derive(Debug)]
pub struct WatchStateStore {
    directory: PathBuf,
    state: WatcherState,
    _lock: File,
}

impl WatchStateStore {
    /// Open a state directory, creating an uninitialized state only with a scope.
    /// # Errors
    /// Rejects busy locks, corrupt existing state, scope changes and failed I/O.
    pub fn open(directory: impl AsRef<Path>, scope: Option<WatcherScope>) -> eyre::Result<Self> {
        if let Some(scope) = &scope {
            scope.validate()?;
        }
        let directory = directory.as_ref().to_owned();
        if scope.is_some() {
            std::fs::create_dir_all(&directory).wrap_err(WatcherError::Persistence)?;
        } else {
            eyre::ensure!(directory.is_dir(), WatcherError::CorruptState);
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join(LOCK_FILE))
            .wrap_err(WatcherError::Persistence)?;
        lock.try_lock()
            .map_err(|_redacted_error| eyre::eyre!(WatcherError::StateBusy))?;
        let snapshot = directory.join(SNAPSHOT_FILE);
        let state = match std::fs::metadata(&snapshot) {
            Ok(metadata) => {
                eyre::ensure!(
                    metadata.is_file() && metadata.len() <= MAX_SNAPSHOT_BYTES,
                    WatcherError::CorruptState
                );
                let text =
                    std::fs::read_to_string(&snapshot).wrap_err(WatcherError::CorruptState)?;
                let state: WatcherState =
                    facet_json::from_str(&text).wrap_err(WatcherError::CorruptState)?;
                state.validate().wrap_err(WatcherError::CorruptState)?;
                if let Some(expected) = scope {
                    eyre::ensure!(state.scope == expected, WatcherError::ScopeMismatch);
                }
                state
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                WatcherState::new(scope.ok_or(WatcherError::CorruptState)?)?
            }
            Err(error) => return Err(error).wrap_err(WatcherError::Persistence),
        };
        Ok(Self {
            directory,
            state,
            _lock: lock,
        })
    }

    /// # Errors
    /// Requires a valid committed snapshot and an available state lock.
    pub fn open_existing(directory: impl AsRef<Path>) -> eyre::Result<Self> {
        Self::open(directory, None)
    }

    #[must_use]
    pub const fn state(&self) -> &WatcherState {
        &self.state
    }

    /// Pending events replay oldest-first with their stable message IDs.
    #[must_use]
    pub fn pending(&self) -> Vec<WatchOutboxEntry> {
        let mut pending: Vec<_> = self
            .state
            .outbox
            .iter()
            .filter(|entry| entry.delivered_receipt.is_none())
            .cloned()
            .collect();
        pending.sort_by_key(|entry| entry.message.id);
        pending
    }

    /// Persist an external receipt only after successful dispatch is verified.
    /// # Errors
    /// Rejects unknown events, empty receipts or conflicting repeated receipts.
    pub fn acknowledge(
        &mut self,
        id: WatcherMessageId,
        receipt: String,
    ) -> eyre::Result<WatchOutboxEntry> {
        WatcherState::validate_receipt(&receipt).wrap_err(WatcherError::InvalidAcknowledgment)?;
        let mut next = self.state.clone();
        let entry = next
            .outbox
            .iter_mut()
            .find(|entry| entry.message.id == id)
            .ok_or(WatcherError::InvalidAcknowledgment)?;
        if let Some(existing) = &entry.delivered_receipt {
            eyre::ensure!(*existing == receipt, WatcherError::InvalidAcknowledgment);
            return Ok(entry.clone());
        }
        entry.delivered_receipt = Some(receipt);
        let delivered = entry.clone();
        self.commit(next, &AtomicBool::new(false))?;
        Ok(delivered)
    }

    /// Fetch a bounded backward sweep from a captured head to the old watermark.
    ///
    /// Every completed page durably advances its resume cursor and queues events
    /// in the same snapshot. Only reaching the old watermark/history end advances
    /// the checkpoint to the captured head. Newer arrivals belong to the next
    /// sweep. Initial default behavior establishes the newest-only baseline and
    /// intentionally ignores history; backfill requires an explicit initial ID.
    /// # Errors
    /// Propagates source failures, invalid pages/context, cancellation and I/O.
    /// Already committed pages remain recoverable after any later error.
    pub async fn sweep<S: WatcherSource>(
        &mut self,
        source: &mut S,
        request: &WatchSweepRequest,
        cancelled: &AtomicBool,
    ) -> eyre::Result<WatchSweepReport> {
        request.validate().wrap_err(WatcherError::InvalidSettings)?;
        Self::check_cancelled(cancelled)?;
        if self.state.initialized {
            eyre::ensure!(
                request.backfill_after_id.is_none(),
                WatcherError::AlreadyInitialized
            );
        } else if let Some(lower) = request.backfill_after_id {
            let mut next = self.state.clone();
            next.initialized = true;
            next.checkpoint = Some(lower);
            self.commit(next, cancelled)?;
        } else {
            let messages = source.page(None, 1).await?;
            Self::check_cancelled(cancelled)?;
            self.validate_page(&messages, None, 1)?;
            let mut next = self.state.clone();
            next.initialized = true;
            next.checkpoint = messages.first().map(|message| message.id);
            self.commit(next, cancelled)?;
            return Ok(self.report(true, 1, 0));
        }

        let mut pages_read = 0_u32;
        let mut queued = 0_u32;
        while pages_read < request.max_pages {
            Self::check_cancelled(cancelled)?;
            let before = self.state.resume.as_ref().map(|resume| resume.before);
            let messages = source.page(before, request.page_size).await?;
            Self::check_cancelled(cancelled)?;
            self.validate_page(&messages, before, request.page_size)?;
            pages_read += 1;

            let captured_head = if let Some(resume) = &self.state.resume {
                resume.captured_head
            } else if let Some(message) = messages.first() {
                if self
                    .state
                    .checkpoint
                    .is_some_and(|lower| message.id <= lower)
                {
                    return Ok(self.report(false, pages_read, queued));
                }
                message.id
            } else {
                return Ok(self.report(false, pages_read, queued));
            };
            let lower = self.state.checkpoint;
            let reached_watermark = messages
                .iter()
                .any(|message| lower.is_some_and(|id| message.id <= id));
            let complete = reached_watermark || messages.len() < usize::from(request.page_size);
            let mut next = self.state.clone();
            let mut page_queued = 0_u32;
            for message in &messages {
                if lower.is_some_and(|id| message.id <= id) {
                    break;
                }
                eyre::ensure!(message.id <= captured_head, WatcherError::InvalidPage);
                if message.is_trigger(&self.state.scope)?
                    && !next
                        .outbox
                        .iter()
                        .any(|entry| entry.message.id == message.id)
                {
                    next.outbox.push(WatchOutboxEntry {
                        message: message.clone(),
                        delivered_receipt: None,
                    });
                    page_queued += 1;
                }
            }
            next.outbox.sort_by_key(|entry| entry.message.id);
            if complete {
                next.checkpoint = Some(captured_head);
                next.resume = None;
            } else {
                let oldest = messages.last().ok_or(WatcherError::InvalidPage)?.id;
                next.resume = Some(WatchSweepResume {
                    captured_head,
                    before: oldest,
                    lower_checkpoint: lower,
                    pages_completed: self
                        .state
                        .resume
                        .as_ref()
                        .map_or(1, |resume| resume.pages_completed.saturating_add(1)),
                });
            }
            self.commit(next, cancelled)?;
            queued = queued.saturating_add(page_queued);
            if complete {
                break;
            }
        }
        Ok(self.report(false, pages_read, queued))
    }

    fn report(&self, baseline_created: bool, pages_read: u32, queued: u32) -> WatchSweepReport {
        WatchSweepReport {
            baseline_created,
            pages_read,
            queued,
            checkpoint: self.state.checkpoint,
            gap: self.state.resume.is_some(),
            pending: self
                .state
                .outbox
                .iter()
                .filter(|entry| entry.delivered_receipt.is_none())
                .count() as u64,
        }
    }

    fn validate_page(
        &self,
        messages: &[WatcherMessage],
        before: Option<WatcherMessageId>,
        limit: u8,
    ) -> eyre::Result<()> {
        eyre::ensure!(
            messages.len() <= usize::from(limit),
            WatcherError::InvalidPage
        );
        let mut previous = before;
        for message in messages {
            message
                .validate_scope(&self.state.scope)
                .wrap_err(WatcherError::ScopeMismatch)?;
            eyre::ensure!(
                previous.is_none_or(|id| message.id < id),
                WatcherError::InvalidPage
            );
            previous = Some(message.id);
        }
        Ok(())
    }

    fn check_cancelled(cancelled: &AtomicBool) -> eyre::Result<()> {
        eyre::ensure!(!cancelled.load(Ordering::Acquire), WatcherError::Cancelled);
        Ok(())
    }

    fn commit(&mut self, mut next: WatcherState, cancelled: &AtomicBool) -> eyre::Result<()> {
        Self::check_cancelled(cancelled)?;
        next.provenance.updated_unix_ms =
            WatchProvenance::now().wrap_err(WatcherError::Persistence)?;
        next.validate().wrap_err(WatcherError::CorruptState)?;
        let encoded = facet_json::to_string_pretty(&next).wrap_err(WatcherError::Persistence)?;
        eyre::ensure!(
            encoded.len() as u64 <= MAX_SNAPSHOT_BYTES,
            WatcherError::Persistence
        );
        let pending = self.directory.join(PENDING_FILE);
        let mut staged = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&pending)
            .wrap_err(WatcherError::Persistence)?;
        staged
            .write_all(encoded.as_bytes())
            .wrap_err(WatcherError::Persistence)?;
        staged.sync_all().wrap_err(WatcherError::Persistence)?;
        drop(staged);
        Self::check_cancelled(cancelled)?;
        std::fs::rename(&pending, self.directory.join(SNAPSHOT_FILE))
            .wrap_err(WatcherError::Persistence)?;
        // Rename is the commit boundary. Keep memory consistent even if a later
        // directory durability request fails; the caller must reopen to reconcile.
        self.state = next;
        #[cfg(unix)]
        File::open(&self.directory)
            .and_then(|directory| directory.sync_all())
            .wrap_err(WatcherError::Persistence)?;
        Ok(())
    }
}
