use crate::cli::archive::sync::PreparedSync;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct SyncMembersArgs;

impl SyncMembersArgs {
    /// # Errors
    ///
    /// This function will return an error if the member sync phase fails.
    pub async fn invoke(
        self,
        prepared: &PreparedSync,
        token: &str,
    ) -> Result<crate::archive::SyncRunSummary> {
        let mut summary =
            crate::archive::run_member_sync(prepared.output_dir.path.as_path(), token).await?;
        summary.checkpoint_path = prepared.state.checkpoint_path.display().to_string();
        Ok(summary)
    }
}
