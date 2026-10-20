use super::CaptureSignal;
use super::DaemonCaptureSource;
use super::DaemonEndpoint;
use super::DaemonHello;
use super::DaemonOptions;
use super::DaemonOutcome;
use super::PROTOCOL_VERSION;
use super::actor::Actor;
use super::actor_message::ActorMessage;
use super::offline_source::OfflineSource;
use crate::watch_consumers::ConsumerStore;
use crate::watcher::WatchStateStore;
use crate::watcher::WatchSweepRequest;
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::sync::mpsc;

/// Run explicit local-only mode. Existing eligible events, claims and acknowledgments
/// remain available, but no credential is acquired and no Discord call occurs.
/// # Errors
/// Rejects a competing daemon, invalid state, IPC security or persistence failure.
pub async fn serve_offline(options: DaemonOptions, cancelled: Arc<AtomicBool>) -> eyre::Result<()> {
    let (sender, receiver) = mpsc::channel(1);
    drop(sender);
    serve_inner::<OfflineSource>(options, None, receiver, cancelled).await
}

/// Run a capture source driven by Gateway notifications, never periodic REST polling.
/// The outer production operation owns Gateway shutdown and a hard fixed-lease watchdog.
/// # Errors
/// Propagates state/transport errors. Unacknowledged claims remain durable.
pub async fn serve_with_capture<S: DaemonCaptureSource>(
    options: DaemonOptions,
    source: S,
    wakeups: mpsc::Receiver<CaptureSignal>,
    cancelled: Arc<AtomicBool>,
) -> eyre::Result<()> {
    serve_inner(options, Some(source), wakeups, cancelled).await
}

async fn serve_inner<S: DaemonCaptureSource>(
    options: DaemonOptions,
    mut source: Option<S>,
    mut wakeups: mpsc::Receiver<CaptureSignal>,
    cancelled: Arc<AtomicBool>,
) -> eyre::Result<()> {
    options.validate()?;
    #[cfg(not(windows))]
    {
        let _ = (source, wakeups, cancelled);
        eyre::bail!("Watcher IPC is supported on Windows only")
    }
    #[cfg(windows)]
    {
        let endpoint = DaemonEndpoint::for_state_dir(&options.state_dir)?;
        std::fs::create_dir_all(&options.state_dir)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(options.state_dir.join("daemon.lock"))?;
        lock.try_lock().map_err(|_busy_lock| {
            eyre::eyre!("A watcher daemon already owns this state directory")
        })?;
        let mut core = WatchStateStore::open(
            options.state_dir.join("capture"),
            Some(options.scope.clone()),
        )?;
        if core.state().initialized && options.backfill_after_id.is_some() {
            eyre::bail!("Backfill is only allowed with a new capture state");
        }
        let store =
            ConsumerStore::open(options.state_dir.join("consumers"), options.scope.clone())?;
        let mut actor = Actor {
            store,
            waiters: BTreeMap::new(),
            scope: options.scope.clone(),
            coverage_complete: false,
            checkpoint: core.state().checkpoint,
            capture: if source.is_some() {
                "recovering"
            } else {
                "offline"
            }
            .into(),
            stopping: false,
        };
        // Repair a crash between completed core capture and consumer publication.
        // Publishing retained IDs again never changes sequence or captured payload.
        actor.publish(&core)?;
        if source.is_some() {
            actor.coverage_complete = false;
        }
        let hello = DaemonHello {
            version: PROTOCOL_VERSION,
            scope: options.scope.clone(),
            lease_expires_unix_ms: options.lease_expires_unix_ms,
        };
        let listener = super::pipe_security::create(endpoint.as_str(), true)?;
        let (send, mut receive) = mpsc::channel(64);
        let transport_stop = Arc::new(AtomicBool::new(false));
        let transport_task = tokio::spawn(super::pipe_server::run(
            listener,
            endpoint.as_str().to_owned(),
            hello,
            send,
            Arc::clone(&transport_stop),
        ));
        let mut transport = super::daemon_transport_task::DaemonTransportTask {
            task: transport_task,
            stopped: Arc::clone(&transport_stop),
        };
        let result = run_actor(
            &options,
            &mut actor,
            &mut core,
            &mut source,
            &mut wakeups,
            &mut receive,
            &cancelled,
            &mut transport.task,
        )
        .await;
        actor.finish_waiters(
            if cancelled.load(Ordering::Acquire) {
                DaemonOutcome::Cancelled
            } else {
                DaemonOutcome::Error
            },
            "Daemon stopped before a new event response. Inspect any durable claim before replay",
        );
        transport_stop.store(true, Ordering::Release);
        if !transport.task.is_finished()
            && tokio::time::timeout(Duration::from_secs(3), &mut transport.task)
                .await
                .is_err()
        {
            transport.task.abort();
            let _ = (&mut transport.task).await;
        }
        drop(lock);
        result
    }
}

#[cfg(windows)]
#[expect(
    clippy::too_many_arguments,
    reason = "Capture state, IPC channels, cancellation and transport are borrowed by one serialized actor owner"
)]
#[expect(
    clippy::too_many_lines,
    reason = "The capture, retry and publication loop keeps state transitions and their ordering together"
)]
async fn run_actor<S: DaemonCaptureSource>(
    options: &DaemonOptions,
    actor: &mut Actor,
    core: &mut WatchStateStore,
    source: &mut Option<S>,
    wakeups: &mut mpsc::Receiver<CaptureSignal>,
    commands: &mut mpsc::Receiver<ActorMessage>,
    cancelled: &AtomicBool,
    transport: &mut tokio::task::JoinHandle<eyre::Result<()>>,
) -> eyre::Result<()> {
    let mut dirty = source.is_some();
    let mut wakeups_open = true;
    let mut failures = 0_u32;
    let mut check = tokio::time::interval(Duration::from_millis(25));
    loop {
        if cancelled.load(Ordering::Acquire) || actor.stopping {
            return Ok(());
        }
        actor
            .waiters
            .retain(|_, waiter| !waiter.response.is_closed());
        if dirty && let Some(capture) = source.as_mut() {
            if capture.check_lease().is_err() {
                actor.capture = "approval-required".into();
                actor.finish_waiters(
                    DaemonOutcome::ApprovalRequired,
                    "Fixed credential lease expired. No automatic acquisition occurred",
                );
                *source = None;
                dirty = false;
                continue;
            }
            actor.coverage_complete = false;
            actor.capture = "recovering".into();
            let request = WatchSweepRequest {
                max_pages: options.max_pages,
                page_size: 100,
                backfill_after_id: if core.state().initialized {
                    None
                } else {
                    options.backfill_after_id
                },
            };
            let result =
                sweep_with_commands(core, capture, &request, actor, commands, cancelled).await;
            match result {
                Ok(report) => {
                    failures = 0;
                    actor.checkpoint = report.checkpoint;
                    if report.gap {
                        tracing::warn!(
                            operation = "watch.daemon",
                            phase = "catch-up-gap",
                            pages_read = report.pages_read,
                            "Exact resume cursor retained. Continuing catch-up internally"
                        );
                        tokio::task::yield_now().await;
                    } else {
                        actor.capture = "live".into();
                        actor.publish(core)?;
                        dirty = false;
                    }
                }
                Err(_) if actor.stopping || cancelled.load(Ordering::Acquire) => return Ok(()),
                Err(_) if capture.check_lease().is_err() => {
                    actor.capture = "approval-required".into();
                    actor.finish_waiters(
                        DaemonOutcome::ApprovalRequired,
                        "Fixed credential lease expired. No automatic acquisition occurred",
                    );
                    *source = None;
                    dirty = false;
                }
                Err(error) => {
                    // Retry only explicitly transient reads. Access rejection and
                    // invalid scope/state must not cause another Discord request.
                    if !matches!(
                        error
                            .downcast_ref::<crate::discord::watch_runtime_error::WatchRuntimeError>(
                            ),
                        Some(crate::discord::watch_runtime_error::WatchRuntimeError::Transient)
                    ) {
                        return Err(error);
                    }
                    failures = failures.saturating_add(1);
                    eyre::ensure!(
                        failures < 5,
                        crate::discord::watch_runtime_error::WatchRuntimeError::RetryExhausted
                    );
                    let delay = Duration::from_secs(1_u64 << (failures - 1));
                    tracing::warn!(
                        operation = "watch.daemon",
                        phase = "read-retry",
                        consecutive_failures = failures,
                        retry_seconds = delay.as_secs(),
                        "Capture read failed. Retrying the interrupted dirty cycle"
                    );
                    delay_with_commands(delay, actor, commands, cancelled).await?;
                }
            }
            continue;
        }
        tokio::select! {
            message = commands.recv() => match message {
                Some(message) => actor.handle(message),
                None => return Err(eyre::eyre!("Watcher IPC command channel closed")),
            },
            wake = wakeups.recv(), if wakeups_open => match wake {
                Some(CaptureSignal::Dirty) => { if source.is_some() { dirty = true; } }
                Some(CaptureSignal::ApprovalRequired) => {
                    actor.capture = "approval-required".into();
                    actor.finish_waiters(DaemonOutcome::ApprovalRequired, "Fixed approval expired. No lease was renewed");
                    *source = None;
                }
                Some(CaptureSignal::Failed) => return Err(eyre::eyre!("Gateway capture failed. Restart explicitly after inspecting diagnostics")),
                None => {
                    wakeups_open = false;
                    if source.is_some() { return Err(eyre::eyre!("Gateway notification channel closed")); }
                }
            },
            result = &mut *transport => {
                return match result { Ok(Ok(())) => Err(eyre::eyre!("Watcher pipe listener stopped")), Ok(Err(error)) => Err(error), Err(_) => Err(eyre::eyre!("Watcher pipe listener failed")) };
            }
            _ = check.tick() => {}
        }
    }
}

async fn sweep_with_commands<S: DaemonCaptureSource>(
    core: &mut WatchStateStore,
    source: &mut S,
    request: &WatchSweepRequest,
    actor: &mut Actor,
    commands: &mut mpsc::Receiver<ActorMessage>,
    cancelled: &AtomicBool,
) -> eyre::Result<crate::watcher::WatchSweepReport> {
    let sweep = core.sweep(source, request, cancelled);
    tokio::pin!(sweep);
    let mut check = tokio::time::interval(Duration::from_millis(25));
    loop {
        if cancelled.load(Ordering::Acquire) || actor.stopping {
            eyre::bail!("Daemon stopped during capture");
        }
        tokio::select! {
            result = &mut sweep => return result,
            message = commands.recv() => {
                if let Some(message) = message { actor.handle(message); } else { eyre::bail!("Daemon request channel closed"); }
            }
            _ = check.tick() => {}
        }
    }
}

async fn delay_with_commands(
    delay: Duration,
    actor: &mut Actor,
    commands: &mut mpsc::Receiver<ActorMessage>,
    cancelled: &AtomicBool,
) -> eyre::Result<()> {
    let sleep = tokio::time::sleep(delay);
    tokio::pin!(sleep);
    let mut check = tokio::time::interval(Duration::from_millis(25));
    loop {
        if cancelled.load(Ordering::Acquire) || actor.stopping {
            return Ok(());
        }
        tokio::select! {
            () = &mut sleep => return Ok(()),
            message = commands.recv() => { if let Some(message) = message { actor.handle(message); } else { eyre::bail!("Daemon request channel closed"); } }
            _ = check.tick() => {}
        }
    }
}
