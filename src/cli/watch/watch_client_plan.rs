/// A local-only preview; constructing this record does not connect to the daemon.
#[derive(Debug, facet::Facet)]
pub struct WatchClientPlan {
    pub state_dir: String,
    pub request: crate::watch_daemon::DaemonRequest,
    pub effects: String,
}
