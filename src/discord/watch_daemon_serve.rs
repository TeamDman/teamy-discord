use super::read_context::ReadContext;
use super::watch_daemon_serve_request::WatchDaemonServeRequest;
use super::watch_gateway::GatewayCapture;
use super::watch_lease_watchdog::WatchLeaseWatchdog;
use super::watch_runtime_error::WatchRuntimeError;
use crate::watch_consumers::ConsumerRequestId;
use crate::watch_daemon::DaemonOutcome;
use crate::watch_daemon::DaemonResponse;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

/// Run one explicit foreground daemon. No stdout, service registration, or credential persistence.
///
/// The initial implementation terminates the whole daemon process at actual SDK
/// expiry, closing all Serenity token copies. Restart explicitly with --offline
/// for local claims, or normally for the unchanged broker approval workflow.
/// # Errors
/// Rejects invalid scope, unbounded credentials, denied approval and capture/IPC failure.
pub async fn serve(
    request: WatchDaemonServeRequest,
    cancelled: Arc<AtomicBool>,
) -> eyre::Result<DaemonResponse> {
    match serve_inner(request, Arc::clone(&cancelled)).await {
        Err(error)
            if error.downcast_ref::<WatchRuntimeError>()
                == Some(&WatchRuntimeError::ApprovalRequired) =>
        {
            response(
                DaemonOutcome::ApprovalRequired,
                "Original broker lease expired; explicitly restart for normal approval. No lease was renewed",
            )
        }
        Err(error)
            if error.downcast_ref::<WatchRuntimeError>() == Some(&WatchRuntimeError::Cancelled) =>
        {
            response(
                DaemonOutcome::Cancelled,
                "Watcher daemon cancelled before startup completed",
            )
        }
        result => result,
    }
}

async fn serve_inner(
    request: WatchDaemonServeRequest,
    cancelled: Arc<AtomicBool>,
) -> eyre::Result<DaemonResponse> {
    request.options.validate()?;
    eyre::ensure!(
        !request.read_options.plan,
        "A planned request cannot start a watcher daemon"
    );
    if cancelled.load(Ordering::Acquire) {
        return result(&cancelled);
    }
    if request.offline {
        crate::watch_daemon::serve_offline(request.options, Arc::clone(&cancelled)).await?;
        return result(&cancelled);
    }
    let context = ReadContext::load(request.read_options)?;
    let scope = request.options.scope.clone();
    let plan = context.message_plan(
        "watch.daemon.serve",
        Some(scope.guild_id.get()),
        Some(scope.channel_id.get()),
        None,
    )?;
    // Exact and stable across process invocations. No PID, cursor, consumer,
    // timestamp or request UUID is allowed into this approval identity.
    let purpose = format!(
        "watch.daemon.v1 guild={} channel={} author={} bot={} max-pages={} page-size=100 capabilities=read-message-history,read-referenced-message,gateway-wake-hints",
        scope.guild_id, scope.channel_id, scope.author_id, scope.bot_id, request.options.max_pages
    );
    let acquire = context.http(
        &purpose,
        Some(scope.guild_id.get()),
        Some(scope.channel_id.get()),
    );
    tokio::pin!(acquire);
    let http = loop {
        tokio::select! {
            acquired = &mut acquire => break acquired.map_err(classify_acquisition)?,
            () = tokio::time::sleep(Duration::from_millis(25)) => {
                if cancelled.load(Ordering::Acquire) { return result(&cancelled); }
            }
        }
    };
    eyre::ensure!(
        http.expires_unix_ms().is_some(),
        "Watcher daemon requires a bounded broker lease"
    );
    let mut options = request.options;
    options.lease_expires_unix_ms = http.expires_unix_ms();
    let http = Arc::new(http);
    // Arm before constructing a Gateway client or any retaining background task.
    WatchLeaseWatchdog::arm(Arc::clone(&http))?.retain_until_process_exit();
    let capture =
        GatewayCapture::connect(http, scope, &context, &plan, Arc::clone(&cancelled)).await?;
    let served = crate::watch_daemon::serve_with_capture(
        options,
        capture.source,
        capture.wakeups,
        Arc::clone(&cancelled),
    )
    .await;
    let shutdown = capture.gateway.shutdown().await;
    served?;
    shutdown?;
    result(&cancelled)
}

fn result(cancelled: &AtomicBool) -> eyre::Result<DaemonResponse> {
    let outcome = if cancelled.load(Ordering::Acquire) {
        DaemonOutcome::Cancelled
    } else {
        DaemonOutcome::Stopped
    };
    response(
        outcome,
        "Watcher daemon stopped; durable captures and consumer claims remain recoverable",
    )
}

fn response(outcome: DaemonOutcome, message: &str) -> eyre::Result<DaemonResponse> {
    let id = ConsumerRequestId::new(uuid::Uuid::new_v4().to_string())?;
    Ok(DaemonResponse::new(id, outcome, message))
}

fn classify_acquisition(error: eyre::Report) -> eyre::Report {
    #[cfg(windows)]
    if error.downcast_ref::<teamy_broker_sdk::FailureKind>()
        == Some(&teamy_broker_sdk::FailureKind::LeaseExpired)
    {
        return WatchRuntimeError::ApprovalRequired.into();
    }
    // These are reviewed static SDK lease errors, not raw diagnostic output.
    if matches!(
        error.to_string().as_str(),
        "lease expired" | "invalid or expired lease" | "lease expired in transit"
    ) {
        WatchRuntimeError::ApprovalRequired.into()
    } else {
        error
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn acquisition_expiry_maps_without_classifying_denial_as_expiry() {
        for message in [
            "lease expired",
            "invalid or expired lease",
            "lease expired in transit",
        ] {
            let error = classify_acquisition(eyre::eyre!(message));
            assert_eq!(
                error.downcast_ref::<WatchRuntimeError>(),
                Some(&WatchRuntimeError::ApprovalRequired)
            );
        }
        let denied = classify_acquisition(eyre::eyre!("credential request denied"));
        assert!(denied.downcast_ref::<WatchRuntimeError>().is_none());
    }
}
