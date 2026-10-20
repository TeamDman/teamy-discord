/// Coalesced dirty notifications are hints. Core REST coverage remains authoritative.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureSignal {
    Dirty,
    ApprovalRequired,
    Failed,
}
