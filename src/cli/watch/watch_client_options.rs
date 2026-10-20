use arbitrary::Arbitrary;
use facet::Facet;
use figue as args;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

/// Noncredential options shared only by local IPC clients.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct WatchClientOptions {
    /// State directory selecting the local daemon; this client never opens Discord credentials.
    #[facet(args::named)]
    pub state_dir: String,
    /// Canonical UUID for an explicit retry; omission generates a fresh request identity.
    #[facet(args::named)]
    pub request_id: Option<String>,
    /// Print a request preview without connecting, creating state or changing a consumer.
    #[facet(args::named, default)]
    pub plan: bool,
}

impl WatchClientOptions {
    /// # Errors
    /// Rejects malformed request IDs and state paths before any IPC connection.
    pub fn into_request(
        self,
        command: crate::watch_daemon::DaemonCommand,
    ) -> eyre::Result<(
        crate::watch_daemon::DaemonEndpoint,
        super::WatchClientPlan,
        bool,
    )> {
        let request_id = crate::watch_consumers::ConsumerRequestId::new(
            self.request_id
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        )?;
        let endpoint = crate::watch_daemon::DaemonEndpoint::for_state_dir(std::path::Path::new(
            &self.state_dir,
        ))?;
        let request = crate::watch_daemon::DaemonRequest {
            version: crate::watch_daemon::PROTOCOL_VERSION,
            request_id,
            command,
        };
        request.validate()?;
        let plan = super::WatchClientPlan {
            state_dir: self.state_dir,
            request,
            effects: "Plan only: no IPC connection, credential access or state writes. Execution requires a running local watcher daemon.".into(),
        };
        Ok((endpoint, plan, self.plan))
    }

    /// # Errors
    /// Propagates IPC and output failures; unsuccessful outcomes preserve the daemon exit code.
    pub async fn invoke(
        self,
        command: crate::watch_daemon::DaemonCommand,
    ) -> eyre::Result<crate::cli::output::CliOutput> {
        let (endpoint, plan, plan_only) = self.into_request(command)?;
        if plan_only {
            super::render_response::render_response(&mut std::io::stdout().lock(), &plan)?;
            return Ok(crate::cli::output::CliOutput::none(None));
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        let signal_cancelled = Arc::clone(&cancelled);
        let signal = tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                signal_cancelled.store(true, Ordering::Release);
            }
        });
        let result =
            crate::watch_daemon::client_request(&endpoint, &plan.request, &cancelled).await;
        signal.abort();
        let _ = signal.await;
        let response = result.inspect_err(|_error| {
            tracing::error!(
                source = "watcher-ipc",
                request_id = plan.request.request_id.as_str(),
                outcome = "transport-error",
                "Watcher IPC exchange failed before a complete validated response"
            );
        })?;
        report_response_outcome(&response);
        super::render_response::render_response(&mut std::io::stdout().lock(), &response)?;
        if response.exit_code() != 0 {
            return Err(super::WatchCliExitCode::new(response.exit_code()).into());
        }
        Ok(crate::cli::output::CliOutput::none(None))
    }
}

// These summaries are generated locally from the validated outcome enum. Peer-supplied
// message strings, captured Discord payloads and acknowledgment receipts are never logged.
fn report_response_outcome(response: &crate::watch_daemon::DaemonResponse) {
    use crate::watch_daemon::DaemonOutcome;
    let (outcome, message) = match response.outcome {
        DaemonOutcome::Matched => ("matched", "Watcher event delivered"),
        DaemonOutcome::Replayed => ("replayed", "Watcher delivery explicitly replayed"),
        DaemonOutcome::AlreadyCompleted => (
            "already-completed",
            "Watcher request already completed; inspect the acknowledgment metadata",
        ),
        DaemonOutcome::PendingAck => (
            "pending-ack",
            "Watcher consumer has an unacknowledged delivery; inspect or explicitly replay it",
        ),
        DaemonOutcome::Busy => ("busy", "Watcher consumer already has a pending wait"),
        DaemonOutcome::ConsumerCreated => (
            "consumer-created",
            "Watcher consumer created or its existing cursor restored",
        ),
        DaemonOutcome::Inspected => ("inspected", "Watcher consumer metadata received"),
        DaemonOutcome::Acknowledged => ("acknowledged", "Watcher delivery acknowledged"),
        DaemonOutcome::Status => ("status", "Watcher daemon status received"),
        DaemonOutcome::Stopped => ("stopped", "Watcher daemon stopped"),
        DaemonOutcome::ApprovalRequired => (
            "approval-required",
            "Watcher approval expired or is unavailable; restart the daemon explicitly for broker approval",
        ),
        DaemonOutcome::Offline => ("offline", "Watcher daemon is offline"),
        DaemonOutcome::Cancelled => (
            "cancelled",
            "Watcher wait cancelled; inspect consumer claim state before replaying a delivery",
        ),
        DaemonOutcome::Error => (
            "error",
            "Watcher daemon request failed; inspect local daemon diagnostics",
        ),
    };
    match response.outcome {
        DaemonOutcome::ApprovalRequired | DaemonOutcome::Offline | DaemonOutcome::Error => {
            tracing::error!(
                source = "daemon",
                request_id = response.request_id.as_str(),
                outcome,
                "{message}"
            );
        }
        DaemonOutcome::AlreadyCompleted | DaemonOutcome::PendingAck | DaemonOutcome::Busy => {
            tracing::warn!(
                source = "daemon",
                request_id = response.request_id.as_str(),
                outcome,
                "{message}"
            );
        }
        _ => {
            tracing::info!(
                source = "daemon",
                request_id = response.request_id.as_str(),
                outcome,
                "{message}"
            );
        }
    }
}
