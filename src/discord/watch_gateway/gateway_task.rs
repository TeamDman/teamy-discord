use crate::discord::leased_http::LeasedHttp;
use crate::discord::watch_runtime_error::WatchRuntimeError;
use crate::watch_daemon::CaptureSignal;
use serenity::all::Client;
use serenity::gateway::ShardManager;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Owns cooperative shutdown. The daemon's independent original-lease watchdog
/// remains mandatory: Serenity's detached tasks retain credential copies.
pub struct GatewayTask {
    shutdown_requested: Arc<AtomicBool>,
    manager: Arc<ShardManager>,
    worker: Option<JoinHandle<eyre::Result<()>>>,
}

impl std::fmt::Debug for GatewayTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GatewayTask([redacted connection])")
    }
}

impl GatewayTask {
    pub(super) fn start(
        mut client: Client,
        http: Arc<LeasedHttp>,
        cancelled: Arc<AtomicBool>,
        invalid_identity: Arc<AtomicBool>,
        signals: mpsc::Sender<CaptureSignal>,
    ) -> Self {
        let shutdown_requested = Arc::new(AtomicBool::new(false));
        let task_shutdown = Arc::clone(&shutdown_requested);
        let manager = Arc::clone(&client.shard_manager);
        let worker = tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_millis(10));
            let result = {
                let started = client.start();
                tokio::pin!(started);
                loop {
                    tokio::select! {
                        biased;
                        _ = tick.tick() => {
                            if task_shutdown.load(Ordering::Acquire)
                                || cancelled.load(Ordering::Acquire)
                            {
                                break Ok(());
                            }
                            if http.check().is_err() {
                                break Err(eyre::eyre!(WatchRuntimeError::ApprovalRequired));
                            }
                            if invalid_identity.load(Ordering::Acquire) {
                                break Err(eyre::eyre!(WatchRuntimeError::InvalidBot));
                            }
                        }
                        result = &mut started => {
                            // An unrequested successful return also means the
                            // Gateway is no longer watching. Wake the actor as
                            // a failure instead of leaving it silently idle.
                            let _ = result;
                            break if task_shutdown.load(Ordering::Acquire)
                                || cancelled.load(Ordering::Acquire)
                            {
                                Ok(())
                            } else {
                                Err(eyre::eyre!(WatchRuntimeError::Transient))
                            };
                        }
                    }
                }
            };
            // Best-effort graceful cleanup has a separate finite budget. The
            // whole daemon exits at lease expiry even if this work stalls.
            let _ =
                tokio::time::timeout(Duration::from_secs(2), client.shard_manager.shutdown_all())
                    .await;
            if let Err(error) = &result {
                let signal = if matches!(
                    error.downcast_ref::<WatchRuntimeError>(),
                    Some(WatchRuntimeError::ApprovalRequired)
                ) {
                    CaptureSignal::ApprovalRequired
                } else {
                    CaptureSignal::Failed
                };
                // Terminal state must follow any coalesced dirty wake, rather
                // than being discarded because the capacity-one slot is full.
                let _ = signals.send(signal).await;
            }
            result
        });
        Self {
            shutdown_requested,
            manager,
            worker: Some(worker),
        }
    }

    /// Stop the owned Gateway without renewing approval or touching the broker.
    /// # Errors
    /// Returns a safe failure if its worker failed, panicked, or cleanup stalled.
    pub async fn shutdown(mut self) -> eyre::Result<()> {
        self.shutdown_requested.store(true, Ordering::Release);
        let _ = tokio::time::timeout(Duration::from_secs(2), self.manager.shutdown_all()).await;
        let Some(mut worker) = self.worker.take() else {
            return Ok(());
        };
        match tokio::time::timeout(Duration::from_secs(3), &mut worker).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(WatchRuntimeError::Transient.into()),
            Err(_) => {
                worker.abort();
                let _ = worker.await;
                Err(WatchRuntimeError::Transient.into())
            }
        }
    }
}

impl Drop for GatewayTask {
    fn drop(&mut self) {
        // Drop never blocks. The worker observes this and requests shard
        // shutdown; the independently armed process watchdog is not disarmed.
        self.shutdown_requested.store(true, Ordering::Release);
    }
}
