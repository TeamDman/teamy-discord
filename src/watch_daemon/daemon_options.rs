use crate::watcher::WatcherMessageId;
use crate::watcher::WatcherScope;
use std::path::PathBuf;
#[derive(Clone, Debug)]
pub struct DaemonOptions {
    pub state_dir: PathBuf,
    pub scope: WatcherScope,
    pub max_pages: u32,
    pub backfill_after_id: Option<WatcherMessageId>,
    /// Original broker deadline for the handshake, never a renewed duration.
    pub lease_expires_unix_ms: Option<u64>,
}
impl DaemonOptions {
    /// # Errors
    /// Rejects invalid IDs and bounded capture settings.
    pub fn validate(&self) -> eyre::Result<()> {
        self.scope.validate()?;
        eyre::ensure!(
            !self.state_dir.as_os_str().is_empty()
                && self.state_dir.is_absolute()
                && (1..=1000).contains(&self.max_pages),
            "Invalid daemon settings"
        );
        Ok(())
    }
}
