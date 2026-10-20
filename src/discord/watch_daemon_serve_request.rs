use super::read_options::ReadOptions;
use crate::watch_daemon::DaemonOptions;

/// Typed daemon intent, independent of the CLI's argument schema.
#[derive(Debug)]
pub struct WatchDaemonServeRequest {
    pub read_options: ReadOptions,
    pub options: DaemonOptions,
    pub offline: bool,
}
