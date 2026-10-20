use super::WatcherMessageId;

#[derive(Clone, Debug, Eq, PartialEq, facet::Facet)]
pub struct WatchSweepRequest {
    pub max_pages: u32,
    pub page_size: u8,
    /// First start only: explicitly scan every message newer than this ID.
    pub backfill_after_id: Option<WatcherMessageId>,
}

impl Default for WatchSweepRequest {
    fn default() -> Self {
        Self {
            max_pages: 100,
            page_size: 100,
            backfill_after_id: None,
        }
    }
}

impl WatchSweepRequest {
    /// # Errors
    /// Rejects an unbounded/empty request or invalid Discord page size.
    pub fn validate(&self) -> eyre::Result<()> {
        eyre::ensure!(
            (1..=1000).contains(&self.max_pages),
            "Watcher max-pages must be between 1 and 1000"
        );
        eyre::ensure!(
            (1..=100).contains(&self.page_size),
            "Watcher page-size must be between 1 and 100"
        );
        if let Some(id) = self.backfill_after_id {
            WatcherMessageId::new(id.get())?;
        }
        Ok(())
    }
}
