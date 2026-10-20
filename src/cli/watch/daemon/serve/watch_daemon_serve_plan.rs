/// Credential-free preview of the daemon's fixed capture scope and bounded startup work.
#[derive(Debug, facet::Facet)]
pub struct WatchDaemonServePlan {
    pub query: crate::discord::query_plan::QueryPlan,
    pub scope: crate::watcher::WatcherScope,
    pub state_dir: String,
    pub max_pages: u32,
    pub backfill_after_id: Option<crate::watcher::WatcherMessageId>,
    pub offline: bool,
    pub effects: String,
}
