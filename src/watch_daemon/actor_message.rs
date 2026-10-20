use super::DaemonRequest;
use super::DaemonResponse;
use tokio::sync::oneshot;
#[derive(Debug)]
pub(super) enum ActorMessage {
    Request {
        connection_id: super::connection_id::ConnectionId,
        request: DaemonRequest,
        response: oneshot::Sender<DaemonResponse>,
    },
    Disconnected {
        connection_id: super::connection_id::ConnectionId,
    },
}
