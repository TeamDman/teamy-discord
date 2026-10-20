/// Process-local connection identity is distinct from the retryable request UUID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ConnectionId(pub u64);
