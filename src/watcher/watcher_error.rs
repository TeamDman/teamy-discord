/// Stable, nonsecret operation categories for front ends to classify safely.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WatcherError {
    StateBusy,
    CorruptState,
    ScopeMismatch,
    InvalidPage,
    InvalidReply,
    InvalidSettings,
    AlreadyInitialized,
    InvalidAcknowledgment,
    Persistence,
    Cancelled,
}

impl WatcherError {
    /// Safe diagnostics deliberately exclude transport errors and source content.
    #[must_use]
    pub const fn message(&self) -> &'static str {
        match self {
            Self::StateBusy => {
                "Watcher state is locked by another operation; retry after its sweep completes"
            }
            Self::CorruptState => {
                "Watcher state is invalid; preserve the snapshot and investigate before restarting"
            }
            Self::ScopeMismatch => "Watcher identity or channel differs from the configured scope",
            Self::InvalidPage => "Watcher source returned an invalid or nonadvancing page",
            Self::InvalidReply => {
                "Watcher reply context does not match the referenced message and channel"
            }
            Self::InvalidSettings => "Watcher settings or identifiers are invalid",
            Self::AlreadyInitialized => {
                "Watcher backfill is only allowed when establishing a new state directory"
            }
            Self::InvalidAcknowledgment => {
                "Watcher acknowledgment requires an existing event and a consistent nonsecret receipt"
            }
            Self::Persistence => {
                "Watcher state persistence failed; reopen the state to reconcile committed progress"
            }
            Self::Cancelled => "Watcher operation was cancelled at a safe snapshot boundary",
        }
    }
}

impl std::fmt::Display for WatcherError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for WatcherError {}
