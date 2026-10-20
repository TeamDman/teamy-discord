use crate::cli::archive::sync::PreparedSync;
use crate::cli::archive::sync::SyncCheckpointRestoreArgs;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
use figue::{self as args};
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct SyncCheckpointArgs {
    /// The checkpoint subcommand to run.
    #[facet(args::subcommand)]
    pub command: SyncCheckpointCommand,
}

impl SyncCheckpointArgs {
    /// # Errors
    ///
    /// This function will return an error if the checkpoint subcommand fails.
    pub fn invoke(
        self,
        prepared: &PreparedSync,
    ) -> Result<crate::archive::SyncCheckpointRestoreSummary> {
        match self.command {
            SyncCheckpointCommand::Restore(args) => args.invoke(prepared),
        }
    }
}

#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum SyncCheckpointCommand {
    /// Reconstruct a checkpoint from archived output on disk.
    Restore(SyncCheckpointRestoreArgs),
}
