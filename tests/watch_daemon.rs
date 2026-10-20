#![cfg(windows)]
//! Synthetic IPC/capture fixtures. No credential or Discord network access.
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use teamy_discord::discord::watch_runtime_error::WatchRuntimeError;
use teamy_discord::watch_consumers::ConsumerName;
use teamy_discord::watch_consumers::ConsumerRequestId;
use teamy_discord::watch_consumers::ConsumerStart;
use teamy_discord::watch_daemon::CaptureSignal;
use teamy_discord::watch_daemon::DaemonCaptureSource;
use teamy_discord::watch_daemon::DaemonCommand;
use teamy_discord::watch_daemon::DaemonEndpoint;
use teamy_discord::watch_daemon::DaemonOptions;
use teamy_discord::watch_daemon::DaemonOutcome;
use teamy_discord::watch_daemon::DaemonRequest;
use teamy_discord::watch_daemon::DaemonResponse;
use teamy_discord::watch_daemon::PROTOCOL_VERSION;
use teamy_discord::watch_daemon::client_request;
use teamy_discord::watch_daemon::serve_offline;
use teamy_discord::watch_daemon::serve_with_capture;
use teamy_discord::watcher::WatchStateStore;
use teamy_discord::watcher::WatchSweepRequest;
use teamy_discord::watcher::WatcherChannelId;
use teamy_discord::watcher::WatcherGuildId;
use teamy_discord::watcher::WatcherMessage;
use teamy_discord::watcher::WatcherMessageId;
use teamy_discord::watcher::WatcherScope;
use teamy_discord::watcher::WatcherSource;
use teamy_discord::watcher::WatcherUserId;
use tokio::sync::mpsc;

#[derive(Clone, Debug)]
struct FixtureSource {
    messages: Arc<Mutex<Vec<WatcherMessage>>>,
}
impl FixtureSource {
    fn new(messages: Vec<WatcherMessage>) -> Self {
        Self {
            messages: Arc::new(Mutex::new(messages)),
        }
    }
}
impl WatcherSource for FixtureSource {
    async fn page(
        &mut self,
        before: Option<WatcherMessageId>,
        limit: u8,
    ) -> eyre::Result<Vec<WatcherMessage>> {
        let mut result: Vec<_> = self
            .messages
            .lock()
            .unwrap()
            .iter()
            .filter(|message| before.is_none_or(|before| message.id < before))
            .cloned()
            .collect();
        result.sort_by_key(|message| std::cmp::Reverse(message.id));
        result.truncate(usize::from(limit));
        Ok(result)
    }
    async fn get(&mut self, _id: WatcherMessageId) -> eyre::Result<WatcherMessage> {
        panic!("Fixture has no replies")
    }
}
impl DaemonCaptureSource for FixtureSource {
    fn check_lease(&self) -> eyre::Result<()> {
        Ok(())
    }
}
fn scope() -> WatcherScope {
    WatcherScope {
        guild_id: WatcherGuildId::new(10).unwrap(),
        channel_id: WatcherChannelId::new(20).unwrap(),
        author_id: WatcherUserId::new(30).unwrap(),
        bot_id: WatcherUserId::new(40).unwrap(),
    }
}
fn message(id: u64) -> WatcherMessage {
    WatcherMessage {
        id: WatcherMessageId::new(id).unwrap(),
        guild_id: scope().guild_id,
        channel_id: scope().channel_id,
        author_id: scope().author_id,
        author_is_bot: false,
        is_webhook: false,
        mentioned_user_ids: vec![scope().bot_id],
        content: "Synthetic authorized fixture".into(),
        reply_to: None,
        reply: None,
    }
}
fn options(dir: &Path) -> DaemonOptions {
    DaemonOptions {
        state_dir: dir.to_owned(),
        scope: scope(),
        max_pages: 1,
        backfill_after_id: None,
        lease_expires_unix_ms: None,
    }
}
fn name(value: &str) -> ConsumerName {
    ConsumerName::new(value).unwrap()
}
fn request(command: DaemonCommand) -> DaemonRequest {
    DaemonRequest {
        version: PROTOCOL_VERSION,
        request_id: ConsumerRequestId::new(uuid::Uuid::new_v4().to_string()).unwrap(),
        command,
    }
}
async fn call(endpoint: &DaemonEndpoint, request: &DaemonRequest) -> DaemonResponse {
    tokio::time::timeout(
        Duration::from_secs(8),
        client_request(endpoint, request, &AtomicBool::new(false)),
    )
    .await
    .unwrap()
    .unwrap()
}

#[derive(Debug)]
struct TestDaemon {
    endpoint: DaemonEndpoint,
    cancelled: Arc<AtomicBool>,
    task: tokio::task::JoinHandle<eyre::Result<()>>,
}
impl TestDaemon {
    async fn offline(dir: &Path) -> Self {
        let config = options(dir);
        let endpoint = DaemonEndpoint::for_state_dir(dir).unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        let task_cancel = Arc::clone(&cancelled);
        let task = tokio::spawn(serve_offline(config, task_cancel));
        let daemon = Self {
            endpoint,
            cancelled,
            task,
        };
        daemon.ready().await;
        daemon
    }
    async fn live(dir: &Path, source: FixtureSource) -> (Self, mpsc::Sender<CaptureSignal>) {
        let config = options(dir);
        let endpoint = DaemonEndpoint::for_state_dir(dir).unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = mpsc::channel(1);
        let task = tokio::spawn(serve_with_capture(
            config,
            source,
            receiver,
            Arc::clone(&cancelled),
        ));
        let daemon = Self {
            endpoint,
            cancelled,
            task,
        };
        daemon.ready().await;
        (daemon, sender)
    }
    async fn ready(&self) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let result = call(&self.endpoint, &request(DaemonCommand::Status)).await;
            if result.outcome == DaemonOutcome::Status
                && result
                    .status
                    .as_ref()
                    .is_some_and(|status| status.coverage_complete)
            {
                return;
            }
            assert!(!self.task.is_finished(), "Daemon failed before readiness");
            assert!(
                tokio::time::Instant::now() < deadline,
                "Daemon readiness timeout"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
    async fn stop(&mut self) {
        let response = call(&self.endpoint, &request(DaemonCommand::Stop)).await;
        assert_eq!(response.outcome, DaemonOutcome::Stopped);
        tokio::time::timeout(Duration::from_secs(4), &mut self.task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(
            !self.cancelled.load(Ordering::Acquire),
            "Normal stop must not become user cancellation"
        );
    }
}
impl Drop for TestDaemon {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        self.task.abort();
    }
}

async fn retained_event(dir: &Path) {
    let mut core = WatchStateStore::open(dir.join("capture"), Some(scope())).unwrap();
    let mut source = FixtureSource::new(vec![message(10)]);
    core.sweep(
        &mut source,
        &WatchSweepRequest::default(),
        &AtomicBool::new(false),
    )
    .await
    .unwrap();
    source.messages.lock().unwrap().insert(0, message(20));
    core.sweep(
        &mut source,
        &WatchSweepRequest::default(),
        &AtomicBool::new(false),
    )
    .await
    .unwrap();
}
async fn create(endpoint: &DaemonEndpoint, consumer: &str, start: ConsumerStart) {
    assert_eq!(
        call(
            endpoint,
            &request(DaemonCommand::CreateConsumer {
                consumer: name(consumer),
                start
            })
        )
        .await
        .outcome,
        DaemonOutcome::ConsumerCreated
    );
}

#[tokio::test]
async fn offline_roundtrip_claim_replay_ack_restart_and_independent_consumers() {
    let dir = tempfile::tempdir().unwrap();
    retained_event(dir.path()).await;
    let mut daemon = TestDaemon::offline(dir.path()).await;
    create(&daemon.endpoint, "main", ConsumerStart::Oldest).await;
    create(&daemon.endpoint, "other", ConsumerStart::Oldest).await;
    let next = request(DaemonCommand::Next(name("main")));
    let matched = call(&daemon.endpoint, &next).await;
    assert_eq!(matched.outcome, DaemonOutcome::Matched);
    let delivery = matched.delivery.unwrap();
    assert_eq!(delivery.message.id.get(), 20);
    let retry = call(&daemon.endpoint, &next).await;
    assert_eq!(retry.outcome, DaemonOutcome::PendingAck);
    assert!(retry.delivery.is_none());
    let pending = call(
        &daemon.endpoint,
        &request(DaemonCommand::Next(name("main"))),
    )
    .await;
    assert_eq!(pending.outcome, DaemonOutcome::PendingAck);
    assert!(pending.delivery.is_none());
    let replay = call(
        &daemon.endpoint,
        &request(DaemonCommand::Replay {
            consumer: name("main"),
            delivery_id: delivery.claim.delivery_id.clone(),
        }),
    )
    .await;
    assert_eq!(replay.outcome, DaemonOutcome::Replayed);
    assert_eq!(replay.delivery.unwrap(), delivery);
    daemon.stop().await;
    let mut daemon = TestDaemon::offline(dir.path()).await;
    assert_eq!(
        call(&daemon.endpoint, &next).await.outcome,
        DaemonOutcome::PendingAck
    );
    let ack = request(DaemonCommand::Ack {
        consumer: name("main"),
        event_id: delivery.message.id,
        delivery_id: delivery.claim.delivery_id.clone(),
        receipt: "fixture:completed".into(),
    });
    for _ in 0..2 {
        assert_eq!(
            call(&daemon.endpoint, &ack).await.outcome,
            DaemonOutcome::Acknowledged
        );
    }
    let completed = call(&daemon.endpoint, &next).await;
    assert_eq!(completed.outcome, DaemonOutcome::AlreadyCompleted);
    assert!(completed.delivery.is_none());
    assert_eq!(completed.exit_code(), 1);
    assert_eq!(
        call(
            &daemon.endpoint,
            &request(DaemonCommand::Next(name("other")))
        )
        .await
        .outcome,
        DaemonOutcome::Matched
    );
    daemon.stop().await;
}

#[tokio::test]
async fn fresh_offline_consumer_uses_explicit_empty_published_boundary() {
    let dir = tempfile::tempdir().unwrap();
    let mut daemon = TestDaemon::offline(dir.path()).await;
    create(&daemon.endpoint, "main", ConsumerStart::Now).await;
    assert_eq!(
        call(
            &daemon.endpoint,
            &request(DaemonCommand::Next(name("main")))
        )
        .await
        .outcome,
        DaemonOutcome::Offline
    );
    daemon.stop().await;
}

#[tokio::test]
async fn same_consumer_busy_client_cannot_cancel_original_waiter_and_dirty_is_event_driven() {
    let dir = tempfile::tempdir().unwrap();
    let source = FixtureSource::new(vec![message(10)]);
    let data = Arc::clone(&source.messages);
    let (mut daemon, wakeups) = TestDaemon::live(dir.path(), source).await;
    create(&daemon.endpoint, "main", ConsumerStart::Now).await;
    let first_request = request(DaemonCommand::Next(name("main")));
    let endpoint = daemon.endpoint.clone();
    let request_copy = first_request.clone();
    let mut first = tokio::spawn(async move { call(&endpoint, &request_copy).await });
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        let status = call(&daemon.endpoint, &request(DaemonCommand::Status))
            .await
            .status
            .unwrap();
        if status.waiting_consumers == 1 {
            break;
        }
        if first.is_finished() {
            let response = (&mut first).await.unwrap();
            panic!(
                "Next completed before waiter registration: {:?}: {}",
                response.outcome, response.message
            );
        }
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    // Same UUID on a second connection still gets Busy, never cancels the first.
    assert_eq!(
        call(&daemon.endpoint, &first_request).await.outcome,
        DaemonOutcome::Busy
    );
    assert!(!first.is_finished());
    data.lock().unwrap().insert(0, message(20));
    wakeups.send(CaptureSignal::Dirty).await.unwrap();
    let response = tokio::time::timeout(Duration::from_secs(3), first)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(response.outcome, DaemonOutcome::Matched);
    assert_eq!(response.delivery.unwrap().message.id.get(), 20);
    daemon.stop().await;
}

#[tokio::test]
async fn cancellation_removes_only_the_waiting_connection_without_stopping_daemon() {
    let dir = tempfile::tempdir().unwrap();
    let source = FixtureSource::new(vec![message(10)]);
    let (mut daemon, _wakeups) = TestDaemon::live(dir.path(), source).await;
    create(&daemon.endpoint, "main", ConsumerStart::Now).await;
    let cancelled = Arc::new(AtomicBool::new(false));
    let endpoint = daemon.endpoint.clone();
    let flag = Arc::clone(&cancelled);
    let waiting = tokio::spawn(async move {
        client_request(
            &endpoint,
            &request(DaemonCommand::Next(name("main"))),
            &flag,
        )
        .await
        .unwrap()
    });
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        if call(&daemon.endpoint, &request(DaemonCommand::Status))
            .await
            .status
            .unwrap()
            .waiting_consumers
            == 1
        {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    cancelled.store(true, Ordering::Release);
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), waiting)
            .await
            .unwrap()
            .unwrap()
            .outcome,
        DaemonOutcome::Cancelled
    );
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        if call(&daemon.endpoint, &request(DaemonCommand::Status))
            .await
            .status
            .unwrap()
            .waiting_consumers
            == 0
        {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    daemon.stop().await;
}

#[tokio::test]
async fn second_daemon_rejected_and_aborted_serve_does_not_detach_listener() {
    let dir = tempfile::tempdir().unwrap();
    let mut daemon = TestDaemon::offline(dir.path()).await;
    let competing = serve_offline(options(dir.path()), Arc::new(AtomicBool::new(false))).await;
    assert!(competing.is_err());
    let endpoint = daemon.endpoint.clone();
    daemon.task.abort();
    let aborted = tokio::time::timeout(Duration::from_secs(2), &mut daemon.task)
        .await
        .unwrap();
    assert!(aborted.unwrap_err().is_cancelled());
    tokio::task::yield_now().await;
    // Abort-on-drop transport guard releases all listener handles.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        if call(&endpoint, &request(DaemonCommand::Status))
            .await
            .outcome
            == DaemonOutcome::Error
        {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    drop(daemon);
    let mut restarted = TestDaemon::offline(dir.path()).await;
    restarted.stop().await;
}

#[derive(Debug)]
struct FixtureChild(std::process::Child);
impl Drop for FixtureChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test]
async fn verified_server_exit_three_is_approval_expiry_before_advertised_wall_deadline() {
    use std::os::windows::process::CommandExt;
    let dir = tempfile::tempdir().unwrap();
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "synthetic_exit3_fixture",
            "--ignored",
            "--nocapture",
        ])
        .env("TEAMY_WATCH_EXIT3_FIXTURE", dir.path())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(0x0800_0000)
        .spawn()
        .unwrap();
    let _child = FixtureChild(child);
    let endpoint = DaemonEndpoint::for_state_dir(dir.path()).unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(4);
    loop {
        let status = call(&endpoint, &request(DaemonCommand::Status)).await;
        if status.status.is_some_and(|status| status.coverage_complete) {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Exit fixture readiness timed out"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    create(&endpoint, "main", ConsumerStart::Now).await;
    let response = tokio::time::timeout(
        Duration::from_secs(8),
        client_request(
            &endpoint,
            &request(DaemonCommand::Next(name("main"))),
            &AtomicBool::new(false),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(response.outcome, DaemonOutcome::ApprovalRequired);
    assert_eq!(response.exit_code(), 3);
}

#[tokio::test]
#[ignore = "Spawned only by the bounded server-exit fixture parent"]
async fn synthetic_exit3_fixture() {
    let Some(directory) = std::env::var_os("TEAMY_WATCH_EXIT3_FIXTURE") else {
        return;
    };
    let mut config = options(Path::new(&directory));
    // Deliberately later than actual process termination. EOF classification must
    // use the retained verified process object rather than the wall deadline.
    config.lease_expires_unix_ms = Some(
        u64::try_from(
            (std::time::SystemTime::now() + Duration::from_mins(1))
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        )
        .unwrap(),
    );
    let (sender, receiver) = mpsc::channel(1);
    let _watchdog = std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(5));
        std::process::exit(3);
    });
    let result = serve_with_capture(
        config,
        FixtureSource::new(vec![message(10)]),
        receiver,
        Arc::new(AtomicBool::new(false)),
    )
    .await;
    drop(sender);
    result.unwrap();
}

#[tokio::test]
async fn explicit_stop_is_success_for_stop_command_but_not_for_waiting_next() {
    let dir = tempfile::tempdir().unwrap();
    let (mut daemon, _wakeups) =
        TestDaemon::live(dir.path(), FixtureSource::new(vec![message(10)])).await;
    create(&daemon.endpoint, "main", ConsumerStart::Now).await;
    let endpoint = daemon.endpoint.clone();
    let pending =
        tokio::spawn(
            async move { call(&endpoint, &request(DaemonCommand::Next(name("main")))).await },
        );
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        if call(&daemon.endpoint, &request(DaemonCommand::Status))
            .await
            .status
            .unwrap()
            .waiting_consumers
            == 1
        {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    daemon.stop().await;
    let response = tokio::time::timeout(Duration::from_secs(1), pending)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(response.outcome, DaemonOutcome::Error);
    assert_eq!(response.exit_code(), 1);
    assert!(response.delivery.is_none());
}

#[tokio::test]
async fn bounded_accept_pool_handles_status_pressure_and_independent_next_clients() {
    let dir = tempfile::tempdir().unwrap();
    let source = FixtureSource::new(vec![message(10)]);
    let messages = Arc::clone(&source.messages);
    let (mut daemon, wakeups) = TestDaemon::live(dir.path(), source).await;
    let mut pending = Vec::new();
    for index in 0..4 {
        let consumer = format!("worker-{index}");
        create(&daemon.endpoint, &consumer, ConsumerStart::Now).await;
        let endpoint = daemon.endpoint.clone();
        pending.push(tokio::spawn(async move {
            call(&endpoint, &request(DaemonCommand::Next(name(&consumer)))).await
        }));
    }
    let mut pressure = Vec::new();
    for _ in 0..2 {
        let endpoint = daemon.endpoint.clone();
        pressure.push(tokio::spawn(async move {
            for _ in 0..50 {
                assert_eq!(
                    call(&endpoint, &request(DaemonCommand::Status))
                        .await
                        .outcome,
                    DaemonOutcome::Status
                );
            }
        }));
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    loop {
        let status = call(&daemon.endpoint, &request(DaemonCommand::Status))
            .await
            .status
            .unwrap();
        if status.waiting_consumers == 4 {
            break;
        }
        assert!(
            pending.iter().all(|task| !task.is_finished()),
            "A waiting Next unexpectedly completed under connection pressure"
        );
        assert!(
            tokio::time::Instant::now() < deadline,
            "Independent Next clients were starved by status pressure"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    messages.lock().unwrap().insert(0, message(20));
    wakeups.send(CaptureSignal::Dirty).await.unwrap();
    let mut delivery_ids = std::collections::BTreeSet::new();
    for task in pending {
        let response = tokio::time::timeout(Duration::from_secs(3), task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(response.outcome, DaemonOutcome::Matched);
        let delivery = response.delivery.unwrap();
        assert_eq!(delivery.message.id.get(), 20);
        assert!(delivery_ids.insert(delivery.claim.delivery_id));
    }
    for task in pressure {
        tokio::time::timeout(Duration::from_secs(3), task)
            .await
            .unwrap()
            .unwrap();
    }
    daemon.stop().await;
}

#[derive(Debug)]
struct FailingCaptureSource {
    error: WatchRuntimeError,
    attempts: Arc<AtomicUsize>,
}

impl WatcherSource for FailingCaptureSource {
    async fn page(
        &mut self,
        _before: Option<WatcherMessageId>,
        _limit: u8,
    ) -> eyre::Result<Vec<WatcherMessage>> {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        Err(self.error.into())
    }

    async fn get(&mut self, _id: WatcherMessageId) -> eyre::Result<WatcherMessage> {
        panic!("Failing capture fixture never reads reply context")
    }
}

impl DaemonCaptureSource for FailingCaptureSource {
    fn check_lease(&self) -> eyre::Result<()> {
        Ok(())
    }
}

async fn failed_capture(error: WatchRuntimeError) -> (usize, eyre::Report) {
    let directory = tempfile::tempdir().unwrap();
    let attempts = Arc::new(AtomicUsize::new(0));
    let source = FailingCaptureSource {
        error,
        attempts: Arc::clone(&attempts),
    };
    let (_signals, wakeups) = mpsc::channel(1);
    let result = tokio::time::timeout(
        Duration::from_secs(25),
        serve_with_capture(
            options(directory.path()),
            source,
            wakeups,
            Arc::new(AtomicBool::new(false)),
        ),
    )
    .await
    .expect("Failing capture exceeded its bounded retry deadline")
    .expect_err("Failing capture unexpectedly succeeded");
    (attempts.load(Ordering::SeqCst), result)
}

#[tokio::test]
async fn rejected_capture_access_stops_after_one_request() {
    let (attempts, error) = failed_capture(WatchRuntimeError::AccessRejected).await;
    assert_eq!(attempts, 1);
    assert_eq!(
        error.downcast_ref::<WatchRuntimeError>(),
        Some(&WatchRuntimeError::AccessRejected)
    );
}

#[tokio::test]
async fn transient_capture_failures_stop_after_five_requests() {
    let (attempts, error) = failed_capture(WatchRuntimeError::Transient).await;
    assert_eq!(attempts, 5);
    assert_eq!(
        error.downcast_ref::<WatchRuntimeError>(),
        Some(&WatchRuntimeError::RetryExhausted)
    );
}
