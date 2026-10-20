use crate::cli::archive::sync::PreparedSync;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
use figue::{self as args};
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct SyncCheckpointRestoreArgs {
    /// Reconstruct and compare without writing the checkpoint file.
    #[facet(args::named, default)]
    pub dry_run: bool,
}

impl SyncCheckpointRestoreArgs {
    /// # Errors
    ///
    /// This function will return an error if checkpoint reconstruction or comparison fails.
    pub fn invoke(
        self,
        prepared: &PreparedSync,
    ) -> Result<crate::archive::SyncCheckpointRestoreSummary> {
        super::super::super::sync_cli::restore_sync_checkpoint(prepared, self.dry_run)
    }
}
