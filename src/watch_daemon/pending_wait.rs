use super::DaemonResponse;
use crate::watch_consumers::ConsumerRequestId;
use tokio::sync::oneshot;
#[derive(Debug)]
pub(super) struct PendingWait {
    pub connection_id: super::connection_id::ConnectionId,
    pub request_id: ConsumerRequestId,
    pub response: oneshot::Sender<DaemonResponse>,
}
