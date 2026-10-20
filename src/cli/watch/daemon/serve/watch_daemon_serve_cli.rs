use crate::discord::read_context::ReadContext;
use crate::discord::read_options::ReadOptions;
use crate::watcher::WatcherChannelId;
use crate::watcher::WatcherGuildId;
use crate::watcher::WatcherMessageId;
use crate::watcher::WatcherScope;
use crate::watcher::WatcherUserId;
use arbitrary::Arbitrary;
use facet::Facet;
use figue as args;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

/// Capture one fixed channel, human author and bot under a bounded broker approval lease.
/// A live daemon performs read-only Discord capture; local consumers handle delivery separately.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct WatchDaemonServeArgs {
    /// Profile and approved read options apply only to daemon capture, never to IPC clients.
    #[facet(flatten)]
    pub read_options: ReadOptions,
    /// Absolute state directory; one daemon owns capture and consumer writes here.
    #[facet(args::named)]
    pub state_dir: String,
    /// Guild ID; otherwise use the selected profile default.
    #[facet(args::named)]
    pub guild_id: Option<u64>,
    /// Exact channel or thread ID; otherwise use the selected profile default.
    #[facet(args::named)]
    pub channel_id: Option<u64>,
    /// Human author whose explicit mentions of the configured bot become events.
    #[facet(args::named)]
    pub author_id: u64,
    /// Expected authenticated bot ID; live capture verifies the actual account.
    #[facet(args::named)]
    pub bot_id: u64,
    /// Maximum pages in each bounded capture sweep (default 10; range 1..=1000).
    #[facet(args::named)]
    pub max_pages: Option<u32>,
    /// Explicit lower message-ID boundary for initial backfill; omission starts at now.
    #[facet(args::named)]
    pub backfill_after_id: Option<u64>,
    /// Serve retained local fixture events only, without Discord or broker acquisition.
    #[facet(args::named, default)]
    pub offline: bool,
}

impl WatchDaemonServeArgs {
    /// # Errors
    /// Rejects invalid profile scope, IDs, state paths and capture bounds before any credential access.
    pub fn into_request(
        self,
    ) -> eyre::Result<(
        crate::discord::watch_daemon_serve_request::WatchDaemonServeRequest,
        super::WatchDaemonServePlan,
        bool,
    )> {
        let plan_only = self.read_options.plan;
        let context = ReadContext::load(self.read_options)?;
        let query =
            context.message_plan("watch.daemon.serve", self.guild_id, self.channel_id, None)?;
        let scope = WatcherScope {
            guild_id: WatcherGuildId::new(
                query
                    .guild_id
                    .ok_or_else(|| eyre::eyre!("Missing watcher guild"))?,
            )?,
            channel_id: WatcherChannelId::new(
                query
                    .channel_id
                    .ok_or_else(|| eyre::eyre!("Missing watcher channel"))?,
            )?,
            author_id: WatcherUserId::new(self.author_id)?,
            bot_id: WatcherUserId::new(self.bot_id)?,
        };
        let options = crate::watch_daemon::DaemonOptions {
            state_dir: std::path::PathBuf::from(&self.state_dir),
            scope: scope.clone(),
            max_pages: self.max_pages.unwrap_or(10),
            backfill_after_id: self
                .backfill_after_id
                .map(WatcherMessageId::new)
                .transpose()?,
            lease_expires_unix_ms: None,
        };
        options.validate()?;
        // Endpoint construction validates the absolute path and performs no state writes.
        crate::watch_daemon::DaemonEndpoint::for_state_dir(&options.state_dir)?;
        let plan = super::WatchDaemonServePlan {
            query,
            scope,
            state_dir: self.state_dir,
            max_pages: options.max_pages,
            backfill_after_id: options.backfill_after_id,
            offline: self.offline,
            effects: "Plan only: no credential access, Discord connection, IPC connection or state writes. Live execution requires a fresh bounded broker approval; offline execution serves retained fixture events only.".into(),
        };
        Ok((
            crate::discord::watch_daemon_serve_request::WatchDaemonServeRequest {
                read_options: context.options,
                options,
                offline: self.offline,
            },
            plan,
            plan_only,
        ))
    }

    /// # Errors
    /// Propagates capture, approval, cancellation and checked output failures.
    pub async fn invoke(self) -> eyre::Result<crate::cli::output::CliOutput> {
        let (request, plan, plan_only) = self.into_request()?;
        if plan_only {
            crate::cli::watch::render_response::render_response(
                &mut std::io::stdout().lock(),
                &plan,
            )?;
            return Ok(crate::cli::output::CliOutput::none(None));
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        let signal_cancelled = Arc::clone(&cancelled);
        let signal = tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                signal_cancelled.store(true, Ordering::Release);
            }
        });
        let result = crate::discord::watch_daemon_serve::serve(request, cancelled).await;
        signal.abort();
        let _ = signal.await;
        let response = result?;
        crate::cli::watch::render_response::render_response(
            &mut std::io::stdout().lock(),
            &response,
        )?;
        if response.exit_code() != 0 {
            return Err(crate::cli::watch::WatchCliExitCode::new(response.exit_code()).into());
        }
        Ok(crate::cli::output::CliOutput::none(None))
    }
}
