use super::DaemonCommand;
use super::DaemonOutcome;
use super::DaemonResponse;
use super::DaemonStatus;
use super::actor_message::ActorMessage;
use super::pending_wait::PendingWait;
use crate::watch_consumers::ConsumerClaimResult;
use crate::watch_consumers::ConsumerError;
use crate::watch_consumers::ConsumerName;
use crate::watch_consumers::ConsumerRequestBinding;
use crate::watch_consumers::ConsumerRequestId;
use crate::watch_consumers::ConsumerStore;
use crate::watcher::WatchStateStore;
use crate::watcher::WatcherMessageId;
use crate::watcher::WatcherScope;
use std::collections::BTreeMap;
use tokio::sync::oneshot;

/// Only this actor mutates publication/consumer state or assigns waiting requests.
#[derive(Debug)]
pub(super) struct Actor {
    pub store: ConsumerStore,
    pub waiters: BTreeMap<String, PendingWait>,
    pub scope: WatcherScope,
    pub coverage_complete: bool,
    pub checkpoint: Option<WatcherMessageId>,
    pub capture: String,
    pub stopping: bool,
}
impl Actor {
    pub fn handle(&mut self, message: ActorMessage) {
        match message {
            ActorMessage::Disconnected { connection_id } => {
                self.waiters
                    .retain(|_, waiter| waiter.connection_id != connection_id);
            }
            ActorMessage::Request {
                connection_id,
                request,
                response,
            } => {
                if response.is_closed() {
                    return;
                }
                let id = request.request_id.clone();
                if request.validate().is_err() {
                    let _ = response.send(DaemonResponse::new(
                        id,
                        DaemonOutcome::Error,
                        "Invalid watcher IPC request",
                    ));
                    return;
                }
                self.waiters
                    .retain(|_, waiter| !waiter.response.is_closed());
                if let DaemonCommand::Next(consumer) = request.command {
                    self.next(&consumer, id, connection_id, response);
                    return;
                }
                let result = self.operation(id.clone(), request.command);
                let output = match result {
                    Ok(output) => output,
                    Err(error) => self.failure(id, &error),
                };
                let _ = response.send(output);
            }
        }
    }

    fn operation(
        &mut self,
        id: ConsumerRequestId,
        command: DaemonCommand,
    ) -> eyre::Result<DaemonResponse> {
        match command {
            DaemonCommand::Status => {
                let mut response = DaemonResponse::new(id, DaemonOutcome::Status, "Daemon status");
                response.status = Some(DaemonStatus {
                    scope: self.scope.clone(),
                    capture: self.capture.clone(),
                    coverage_complete: self.coverage_complete,
                    checkpoint: self.checkpoint,
                    waiting_consumers: self.waiters.len() as u64,
                });
                Ok(response)
            }
            DaemonCommand::CreateConsumer { consumer, start } => {
                eyre::ensure!(
                    self.coverage_complete,
                    "Consumer creation requires a completed capture publication boundary"
                );
                self.store.bind_request(
                    id.clone(),
                    ConsumerRequestBinding::create(consumer.clone(), start),
                )?;
                let status = self.store.create(consumer, start)?;
                let mut response = DaemonResponse::new(
                    id,
                    DaemonOutcome::ConsumerCreated,
                    "Consumer created or restored at its retained cursor",
                );
                response.consumer = Some(status);
                Ok(response)
            }
            DaemonCommand::Inspect(consumer) => {
                self.store.bind_request(
                    id.clone(),
                    ConsumerRequestBinding::inspect(consumer.clone()),
                )?;
                let mut response = DaemonResponse::new(
                    id,
                    DaemonOutcome::Inspected,
                    "Only explicit replay returns an existing claim payload",
                );
                response.consumer = Some(self.store.inspect(&consumer)?);
                Ok(response)
            }
            DaemonCommand::Replay {
                consumer,
                delivery_id,
            } => {
                self.store.bind_request(
                    id.clone(),
                    ConsumerRequestBinding::replay(consumer.clone(), delivery_id.clone()),
                )?;
                let mut response = DaemonResponse::new(
                    id,
                    DaemonOutcome::Replayed,
                    "Deliberate replay. Reconcile uncertain external effects before processing again",
                );
                response.delivery = Some(self.store.replay(&consumer, &delivery_id)?);
                Ok(response)
            }
            DaemonCommand::Ack {
                consumer,
                event_id,
                delivery_id,
                receipt,
            } => {
                self.store.bind_request(
                    id.clone(),
                    ConsumerRequestBinding::ack(
                        consumer.clone(),
                        event_id,
                        delivery_id.clone(),
                        receipt.clone(),
                    ),
                )?;
                let ack = self.store.ack(&consumer, event_id, &delivery_id, receipt)?;
                let mut response = DaemonResponse::new(
                    id,
                    DaemonOutcome::Acknowledged,
                    "Acknowledgment durably committed",
                );
                response.acknowledgment = Some(ack);
                Ok(response)
            }
            DaemonCommand::Stop => {
                self.stopping = true;
                self.capture = "stopped".into();
                self.finish_waiters(
                    DaemonOutcome::Error,
                    "Daemon stopped before an event was claimed",
                );
                Ok(DaemonResponse::new(
                    id,
                    DaemonOutcome::Stopped,
                    "Daemon stopping. Durable claims remain unchanged",
                ))
            }
            DaemonCommand::Next(_) => unreachable!("Next is handled with its response channel"),
        }
    }

    fn next(
        &mut self,
        consumer: &ConsumerName,
        id: ConsumerRequestId,
        connection_id: super::connection_id::ConnectionId,
        response: oneshot::Sender<DaemonResponse>,
    ) {
        if self.waiters.contains_key(consumer.as_str()) {
            let _ = response.send(DaemonResponse::new(
                id,
                DaemonOutcome::Busy,
                "This consumer already has an active waiting client",
            ));
            return;
        }
        match self.store.claim(consumer, id.clone()) {
            Ok(ConsumerClaimResult::Empty)
                if self.capture == "live" || self.capture == "recovering" =>
            {
                self.waiters.insert(
                    consumer.as_str().to_owned(),
                    PendingWait {
                        connection_id,
                        request_id: id,
                        response,
                    },
                );
            }
            Ok(result) => {
                let output = self.claim_response(id, result);
                let _ = response.send(output);
            }
            Err(error) => {
                let output = self.failure(id, &error);
                let _ = response.send(output);
            }
        }
    }

    fn claim_response(&self, id: ConsumerRequestId, result: ConsumerClaimResult) -> DaemonResponse {
        match result {
            ConsumerClaimResult::Claimed(delivery) => {
                let mut response = DaemonResponse::new(
                    id,
                    DaemonOutcome::Matched,
                    "Verified top-level author message is the trigger. Other authors and reply content remain context. Output is not acknowledgment or permission for a public reply",
                );
                response.delivery = Some(delivery);
                response
            }
            ConsumerClaimResult::PendingAck(claim) => {
                let mut response = DaemonResponse::new(
                    id,
                    DaemonOutcome::PendingAck,
                    "A durable claim awaits acknowledgment. Inspect and explicitly replay for recovery",
                );
                response.claim = Some(claim);
                response
            }
            ConsumerClaimResult::AlreadyCompleted(ack) => {
                let mut response = DaemonResponse::new(
                    id,
                    DaemonOutcome::AlreadyCompleted,
                    "This request was already completed. No new event was claimed",
                );
                response.acknowledgment = Some(ack);
                response
            }
            ConsumerClaimResult::Empty => {
                if self.capture == "approval-required" {
                    DaemonResponse::new(
                        id,
                        DaemonOutcome::ApprovalRequired,
                        "Fixed credential lease expired. Explicit authorization is required. No lease was renewed",
                    )
                } else {
                    DaemonResponse::new(
                        id,
                        DaemonOutcome::Offline,
                        "No eligible event. This daemon is offline and cannot capture Discord",
                    )
                }
            }
        }
    }

    fn failure(&mut self, id: ConsumerRequestId, error: &eyre::Report) -> DaemonResponse {
        let message = if let Some(kind) = error.downcast_ref::<ConsumerError>() {
            if matches!(
                kind,
                ConsumerError::Persistence
                    | ConsumerError::CorruptState
                    | ConsumerError::ScopeMismatch
            ) {
                self.stopping = true;
            }
            kind.message()
        } else {
            "Watcher operation rejected. Check daemon scope and coverage status"
        };
        DaemonResponse::new(id, DaemonOutcome::Error, message)
    }

    pub fn publish(&mut self, core: &WatchStateStore) -> eyre::Result<()> {
        self.checkpoint = core.state().checkpoint;
        self.coverage_complete = core.state().resume.is_none()
            && (core.state().initialized
                || (self.capture == "offline" && core.state().outbox.is_empty()));
        if !self.coverage_complete {
            return Ok(());
        }
        self.store.publish(
            core.state()
                .outbox
                .iter()
                .map(|entry| entry.message.clone())
                .collect(),
        )?;
        self.deliver_waiters();
        Ok(())
    }

    pub fn deliver_waiters(&mut self) {
        for (name, waiter) in std::mem::take(&mut self.waiters) {
            if waiter.response.is_closed() {
                continue;
            }
            let Ok(consumer) = ConsumerName::new(name.clone()) else {
                continue;
            };
            match self.store.claim(&consumer, waiter.request_id.clone()) {
                Ok(ConsumerClaimResult::Empty)
                    if self.capture == "live" || self.capture == "recovering" =>
                {
                    self.waiters.insert(name, waiter);
                }
                Ok(result) => {
                    let response = self.claim_response(waiter.request_id, result);
                    let _ = waiter.response.send(response);
                }
                Err(error) => {
                    let response = self.failure(waiter.request_id, &error);
                    let _ = waiter.response.send(response);
                }
            }
        }
    }

    pub fn finish_waiters(&mut self, outcome: DaemonOutcome, message: &str) {
        for (_, waiter) in std::mem::take(&mut self.waiters) {
            let _ = waiter
                .response
                .send(DaemonResponse::new(waiter.request_id, outcome, message));
        }
    }
}
