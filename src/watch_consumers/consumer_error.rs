#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsumerError {
    StateBusy,
    CorruptState,
    ScopeMismatch,
    InvalidIdentifier,
    UnknownConsumer,
    RequestConflict,
    InvalidDelivery,
    InvalidReceipt,
    Persistence,
}

impl ConsumerError {
    #[must_use]
    pub const fn message(&self) -> &'static str {
        match self {
            Self::StateBusy => "Consumer state is locked by another process",
            Self::CorruptState => {
                "Consumer state is invalid; preserve it and investigate before restarting"
            }
            Self::ScopeMismatch => "Consumer scope or eligible event identity does not match",
            Self::InvalidIdentifier => "Consumer, request or delivery identifier is invalid",
            Self::UnknownConsumer => "The named consumer has not been explicitly created",
            Self::RequestConflict => {
                "This request ID is already bound to different operation parameters"
            }
            Self::InvalidDelivery => {
                "The consumer event and delivery identity do not match a current or acknowledged claim"
            }
            Self::InvalidReceipt => {
                "Completion receipt must be a consistent nonempty nonsecret identifier without control characters"
            }
            Self::Persistence => {
                "Consumer persistence failed; reopen the store to reconcile committed state"
            }
        }
    }
}

impl std::fmt::Display for ConsumerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for ConsumerError {}
