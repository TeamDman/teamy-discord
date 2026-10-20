use super::GatewayRestSource;
use super::GatewayTask;
use super::gateway_handler::GatewayHandler;
use super::guarded_capture::guarded_capture;
use crate::discord::leased_http::LeasedHttp;
use crate::discord::query_plan::QueryPlan;
use crate::discord::read_context::ReadContext;
use crate::discord::watch_runtime_error::WatchRuntimeError;
use crate::watch_daemon::CaptureSignal;
use crate::watch_daemon::DaemonCaptureSource;
use crate::watcher::WatcherError;
use crate::watcher::WatcherScope;
use serenity::all::ClientBuilder;
use serenity::all::GatewayIntents;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::mpsc;

/// Gateway hints plus an authoritative REST source for the daemon's sole actor.
/// The caller must arm its original-lease whole-process watchdog before connect.
#[derive(Debug)]
pub struct GatewayCapture {
    pub source: GatewayRestSource,
    pub wakeups: mpsc::Receiver<CaptureSignal>,
    pub gateway: GatewayTask,
}

impl GatewayCapture {
    /// Verify the configured bot and channel, then start Gateway wakeups.
    /// Uses GUILDS and `GUILD_MESSAGES` only: `MessageCreate` supplies a wake hint;
    /// trigger text and reply context are fetched through the checked REST path.
    /// # Errors
    /// Rejects unbounded direct tokens, invalid scope/policy, cancellation,
    /// expired approval, a different authenticated bot, and connection failures.
    pub async fn connect(
        http: Arc<LeasedHttp>,
        scope: WatcherScope,
        context: &ReadContext,
        plan: &QueryPlan,
        cancelled: Arc<AtomicBool>,
    ) -> eyre::Result<Self> {
        scope.validate()?;
        eyre::ensure!(
            plan.guild_id == Some(scope.guild_id.get())
                && plan.channel_id == Some(scope.channel_id.get()),
            WatcherError::ScopeMismatch
        );
        eyre::ensure!(
            http.expires_unix_ms().is_some(),
            WatchRuntimeError::InvalidSettings
        );
        let source = GatewayRestSource::connect(
            Arc::clone(&http),
            scope.clone(),
            context,
            plan,
            Arc::clone(&cancelled),
        )
        .await?;
        let (signals, wakeups) = mpsc::channel(1);
        let invalid_identity = Arc::new(AtomicBool::new(false));
        let handler = GatewayHandler {
            http: Arc::clone(&http),
            scope,
            cancelled: Arc::clone(&cancelled),
            invalid_identity: Arc::clone(&invalid_identity),
            signals: signals.clone(),
        };
        let builder = ClientBuilder::new_with_http(
            http.gateway_http()?,
            GatewayIntents::GUILDS | GatewayIntents::GUILD_MESSAGES,
        )
        .event_handler(handler);
        let client = guarded_capture(
            async {
                builder
                    .await
                    .map_err(|_redacted_error| eyre::eyre!(WatchRuntimeError::Transient))
            },
            &cancelled,
            || source.check_lease(),
        )
        .await?;
        source.check_lease()?;
        // Startup catch-up does not wait for READY. READY/RESUMED independently
        // request another sweep, repairing arrivals around connection setup.
        let _ = signals.try_send(CaptureSignal::Dirty);
        let gateway = GatewayTask::start(client, http, cancelled, invalid_identity, signals);
        Ok(Self {
            source,
            wakeups,
            gateway,
        })
    }
}
