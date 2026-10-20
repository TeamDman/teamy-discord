#[derive(Clone, Copy, Debug, Eq, PartialEq, facet::Facet)]
#[facet(rename_all = "kebab-case")]
#[repr(u8)]
pub enum DaemonOutcome {
    Matched,
    Replayed,
    AlreadyCompleted,
    PendingAck,
    Busy,
    ConsumerCreated,
    Inspected,
    Acknowledged,
    Status,
    Stopped,
    ApprovalRequired,
    Offline,
    Cancelled,
    Error,
}
