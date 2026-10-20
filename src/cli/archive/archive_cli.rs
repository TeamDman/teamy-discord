use crate::cli::archive::output_dir::OutputDirArgs;
use crate::cli::archive::sync::SyncArgs;
use arbitrary::Arbitrary;
use facet::Facet;
use figue::{self as args};
/// Local archival operations. Sync contacts Discord and writes archived data.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct ArchiveArgs {
    #[facet(args::subcommand)]
    pub command: ArchiveCommand,
}
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum ArchiveCommand {
    /// Resumable archival and checkpoint operations.
    Sync(SyncArgs),
    /// Inspect or configure the archive output location.
    OutputDir(OutputDirArgs),
}
impl ArchiveArgs {
    /// # Errors
    /// Returns an error for invalid configuration, rejected scope or a failed operation.
    pub async fn invoke(self) -> eyre::Result<crate::cli::output::CliOutput> {
        match self.command {
            ArchiveCommand::Sync(args) => args.invoke().await,
            ArchiveCommand::OutputDir(args) => args.invoke().await,
        }
    }
}
