use crate::cli::archive::sync::PreparedSync;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct SyncMessagesArgs;

impl SyncMessagesArgs {
    /// # Errors
    ///
    /// This function will return an error if the message sync phase fails.
    pub async fn invoke(
        self,
        prepared: &PreparedSync,
        token: &str,
    ) -> Result<crate::archive::SyncRunSummary> {
        crate::archive::run_message_sync(prepared.output_dir.path.as_path(), &prepared.state, token)
            .await
    }
}
