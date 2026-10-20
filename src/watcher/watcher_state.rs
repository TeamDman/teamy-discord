use super::WatchOutboxEntry;
use super::WatchProvenance;
use super::WatchSweepResume;
use super::WatcherMessageId;
use super::WatcherScope;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, PartialEq, facet::Facet)]
pub struct WatcherState {
    pub schema_version: u32,
    pub scope: WatcherScope,
    pub initialized: bool,
    pub checkpoint: Option<WatcherMessageId>,
    pub resume: Option<WatchSweepResume>,
    pub outbox: Vec<WatchOutboxEntry>,
    pub provenance: WatchProvenance,
}

impl WatcherState {
    pub(super) fn new(scope: WatcherScope) -> eyre::Result<Self> {
        scope.validate()?;
        Ok(Self {
            schema_version: 1,
            scope,
            initialized: false,
            checkpoint: None,
            resume: None,
            outbox: Vec::new(),
            provenance: WatchProvenance::new()?,
        })
    }

    /// # Errors
    /// Rejects unsupported schemas, corrupt progress or untrusted outbox records.
    pub fn validate(&self) -> eyre::Result<()> {
        eyre::ensure!(self.schema_version == 1, "Unsupported watcher state schema");
        self.scope.validate()?;
        eyre::ensure!(
            self.initialized
                || (self.checkpoint.is_none() && self.resume.is_none() && self.outbox.is_empty()),
            "Uninitialized watcher has persisted progress"
        );
        if let Some(checkpoint) = self.checkpoint {
            WatcherMessageId::new(checkpoint.get())?;
        }
        if let Some(resume) = &self.resume {
            WatcherMessageId::new(resume.captured_head.get())?;
            WatcherMessageId::new(resume.before.get())?;
            eyre::ensure!(
                resume.lower_checkpoint == self.checkpoint,
                "Watcher resume checkpoint differs from state"
            );
            eyre::ensure!(
                resume.before <= resume.captured_head && resume.pages_completed > 0,
                "Invalid watcher resume cursor"
            );
            if let Some(lower) = resume.lower_checkpoint {
                eyre::ensure!(
                    resume.before > lower,
                    "Watcher resume cursor did not advance within gap"
                );
            }
        }
        let upper = self
            .resume
            .as_ref()
            .map(|resume| resume.captured_head)
            .or(self.checkpoint);
        let mut ids = BTreeSet::new();
        for entry in &self.outbox {
            eyre::ensure!(
                upper.is_some_and(|upper| entry.message.id <= upper),
                "Watcher outbox event exceeds captured progress"
            );
            eyre::ensure!(
                ids.insert(entry.message.id),
                "Duplicate watcher outbox message ID"
            );
            eyre::ensure!(
                entry.message.is_trigger(&self.scope)?,
                "Outbox entry is not an authorized watcher trigger"
            );
            if let Some(receipt) = &entry.delivered_receipt {
                Self::validate_receipt(receipt)?;
            }
        }
        eyre::ensure!(
            !self.provenance.tool.is_empty()
                && !self.provenance.tool_version.is_empty()
                && !self.provenance.source_repository.is_empty()
                && !self.provenance.source_revision.is_empty(),
            "Watcher state is missing provenance"
        );
        Ok(())
    }

    pub(super) fn validate_receipt(receipt: &str) -> eyre::Result<()> {
        eyre::ensure!(
            !receipt.trim().is_empty()
                && receipt.len() <= 4096
                && !receipt.chars().any(char::is_control),
            "Watcher delivery receipt must be a nonempty nonsecret identifier without control characters (max 4096 bytes)"
        );
        Ok(())
    }
}
