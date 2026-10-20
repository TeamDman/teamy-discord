use super::DaemonOutcome;
use super::DaemonStatus;
use super::PROTOCOL_VERSION;
use crate::watch_consumers::ConsumerAcknowledgment;
use crate::watch_consumers::ConsumerClaim;
use crate::watch_consumers::ConsumerDelivery;
use crate::watch_consumers::ConsumerRequestId;
use crate::watch_consumers::ConsumerStatus;
#[derive(Clone, Debug, facet::Facet)]
pub struct DaemonResponse {
    pub version: u32,
    pub request_id: ConsumerRequestId,
    pub outcome: DaemonOutcome,
    pub delivery: Option<ConsumerDelivery>,
    pub claim: Option<ConsumerClaim>,
    pub consumer: Option<ConsumerStatus>,
    pub acknowledgment: Option<ConsumerAcknowledgment>,
    pub status: Option<DaemonStatus>,
    pub message: String,
}
impl DaemonResponse {
    #[must_use]
    pub fn new(request_id: ConsumerRequestId, outcome: DaemonOutcome, message: &str) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            request_id,
            outcome,
            delivery: None,
            claim: None,
            consumer: None,
            acknowledgment: None,
            status: None,
            message: message.into(),
        }
    }
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        match self.outcome {
            DaemonOutcome::Cancelled => 130,
            DaemonOutcome::ApprovalRequired => 3,
            DaemonOutcome::Busy
            | DaemonOutcome::PendingAck
            | DaemonOutcome::AlreadyCompleted
            | DaemonOutcome::Offline
            | DaemonOutcome::Error => 1,
            _ => 0,
        }
    }
}
