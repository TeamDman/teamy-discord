/// Safe categories for watcher diagnostics. Raw HTTP bodies and credentials never become events.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WatchRuntimeError {
    ApprovalRequired,
    InvalidBot,
    AccessRejected,
    Cancelled,
    AlreadyRunning,
    Transient,
    RetryExhausted,
    InvalidSettings,
}
impl WatchRuntimeError {
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::ApprovalRequired => {
                "Watcher approval lease expired or became unavailable. Restart explicitly for fresh broker approval. No lease was extended"
            }
            Self::InvalidBot => "Authenticated account is not the configured watcher bot",
            Self::AccessRejected => {
                "Discord rejected watcher access or the selected message is unavailable"
            }
            Self::Cancelled => {
                "Watcher cancelled. Committed state and unacknowledged events remain recoverable"
            }
            Self::AlreadyRunning => "Another watcher is already running for this state directory",
            Self::Transient => {
                "Discord read failed transiently. Watcher progress remains recoverable"
            }
            Self::RetryExhausted => {
                "Watcher stopped after its configured consecutive read failure limit"
            }
            Self::InvalidSettings => "Invalid watcher arguments. Use watch daemon serve --help",
        }
    }
}
impl std::fmt::Display for WatchRuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}
impl std::error::Error for WatchRuntimeError {}
