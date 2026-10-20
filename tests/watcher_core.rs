use std::collections::VecDeque;
use std::io::BufRead as _;
use std::io::Write as _;
use std::process::Child;
use std::process::Command;
use std::process::ExitStatus;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;
use teamy_discord::watcher::WatchStateStore;
use teamy_discord::watcher::WatchSweepRequest;
use teamy_discord::watcher::WatcherChannelId;
use teamy_discord::watcher::WatcherGuildId;
use teamy_discord::watcher::WatcherMessage;
use teamy_discord::watcher::WatcherMessageId;
use teamy_discord::watcher::WatcherReply;
use teamy_discord::watcher::WatcherScope;
use teamy_discord::watcher::WatcherSource;
use teamy_discord::watcher::WatcherUserId;

// These fixtures deliberately exercise the public store/source boundary together.
struct Source {
    pages: VecDeque<eyre::Result<Vec<WatcherMessage>>>,
    calls: Vec<(Option<WatcherMessageId>, u8)>,
    cancel_after_response: Option<Arc<AtomicBool>>,
}

impl Source {
    fn pages(pages: Vec<Vec<WatcherMessage>>) -> Self {
        Self {
            pages: pages.into_iter().map(Ok).collect(),
            calls: Vec::new(),
            cancel_after_response: None,
        }
    }
}

impl WatcherSource for Source {
    async fn page(
        &mut self,
        before: Option<WatcherMessageId>,
        limit: u8,
    ) -> eyre::Result<Vec<WatcherMessage>> {
        self.calls.push((before, limit));
        let response = self
            .pages
            .pop_front()
            .ok_or_else(|| eyre::eyre!("Unexpected synthetic page request"))?;
        if let Some(cancel) = &self.cancel_after_response {
            cancel.store(true, Ordering::SeqCst);
        }
        response
    }

    async fn get(&mut self, _id: WatcherMessageId) -> eyre::Result<WatcherMessage> {
        eyre::bail!("The sweep must consume already hydrated source messages")
    }
}

fn id(value: u64) -> WatcherMessageId {
    WatcherMessageId::new(value).unwrap()
}

fn scope() -> WatcherScope {
    WatcherScope {
        guild_id: WatcherGuildId::new(10).unwrap(),
        channel_id: WatcherChannelId::new(20).unwrap(),
        author_id: WatcherUserId::new(30).unwrap(),
        bot_id: WatcherUserId::new(40).unwrap(),
    }
}

fn trigger(value: u64) -> WatcherMessage {
    WatcherMessage {
        id: id(value),
        guild_id: WatcherGuildId::new(10).unwrap(),
        channel_id: WatcherChannelId::new(20).unwrap(),
        author_id: WatcherUserId::new(30).unwrap(),
        author_is_bot: false,
        is_webhook: false,
        mentioned_user_ids: vec![WatcherUserId::new(40).unwrap()],
        content: format!("Synthetic request {value}"),
        reply_to: Some(id(value - 1)),
        reply: Some(WatcherReply {
            id: id(value - 1),
            guild_id: WatcherGuildId::new(10).unwrap(),
            channel_id: WatcherChannelId::new(20).unwrap(),
            author_id: WatcherUserId::new(50).unwrap(),
            content: "Synthetic referenced concern".into(),
        }),
    }
}

fn request(max_pages: u32, page_size: u8) -> WatchSweepRequest {
    WatchSweepRequest {
        max_pages,
        page_size,
        backfill_after_id: None,
    }
}

fn pending_ids(store: &WatchStateStore) -> Vec<u64> {
    let mut ids: Vec<_> = store
        .pending()
        .iter()
        .map(|entry| entry.message.id.get())
        .collect();
    ids.sort_unstable();
    ids
}

async fn baseline(store: &mut WatchStateStore, value: u64) {
    let mut source = Source::pages(vec![vec![trigger(value)]]);
    let report = store
        .sweep(&mut source, &request(1, 2), &AtomicBool::new(false))
        .await
        .unwrap();
    assert!(report.baseline_created);
    assert_eq!(report.checkpoint, Some(id(value)));
    assert_eq!(report.queued, 0);
    assert_eq!(source.calls, [(None, 1)]);
}

#[test]
fn zero_discord_ids_are_rejected_at_the_typed_boundary() {
    let _ = WatcherMessageId::new(0).unwrap_err();
    let _ = WatcherGuildId::new(0).unwrap_err();
    let _ = WatcherChannelId::new(0).unwrap_err();
    let _ = WatcherUserId::new(0).unwrap_err();
}

#[tokio::test]
async fn default_start_records_only_a_baseline_and_never_replays_history() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = WatchStateStore::open(dir.path(), Some(scope())).unwrap();
    baseline(&mut store, 100).await;
    assert!(store.state().initialized);
    assert!(store.pending().is_empty());
    drop(store);

    let mut store = WatchStateStore::open_existing(dir.path()).unwrap();
    let mut source = Source::pages(vec![vec![trigger(103), trigger(102), trigger(100)]]);
    let report = store
        .sweep(&mut source, &request(1, 3), &AtomicBool::new(false))
        .await
        .unwrap();
    assert!(!report.baseline_created);
    assert!(!report.gap);
    assert_eq!(report.queued, 2);
    assert_eq!(report.pending, 2);
    assert_eq!(report.checkpoint, Some(id(103)));
    assert_eq!(pending_ids(&store), [102, 103]);
}

#[tokio::test]
async fn empty_baseline_is_initialized_and_later_arrivals_are_queued() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = WatchStateStore::open(dir.path(), Some(scope())).unwrap();
    let mut source = Source::pages(vec![vec![]]);
    let first = store
        .sweep(&mut source, &request(1, 2), &AtomicBool::new(false))
        .await
        .unwrap();
    assert!(first.baseline_created);
    assert!(store.state().initialized);
    assert_eq!(first.checkpoint, None);
    drop(store);

    let mut store = WatchStateStore::open_existing(dir.path()).unwrap();
    let mut source = Source::pages(vec![vec![trigger(102)]]);
    let second = store
        .sweep(&mut source, &request(1, 2), &AtomicBool::new(false))
        .await
        .unwrap();
    assert!(!second.baseline_created);
    assert_eq!(second.checkpoint, Some(id(102)));
    assert_eq!(pending_ids(&store), [102]);
}

#[tokio::test]
async fn explicit_backfill_excludes_its_watermark_and_can_span_multiple_pages() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = WatchStateStore::open(dir.path(), Some(scope())).unwrap();
    let mut source = Source::pages(vec![
        vec![trigger(105), trigger(104)],
        vec![trigger(103), trigger(102)],
        vec![trigger(101), trigger(100)],
    ]);
    let mut backfill = request(3, 2);
    backfill.backfill_after_id = Some(id(100));
    let report = store
        .sweep(&mut source, &backfill, &AtomicBool::new(false))
        .await
        .unwrap();
    assert_eq!(report.pages_read, 3);
    assert!(!report.gap);
    assert_eq!(report.checkpoint, Some(id(105)));
    assert_eq!(pending_ids(&store), [101, 102, 103, 104, 105]);
    assert_eq!(
        source.calls,
        [(None, 2), (Some(id(104)), 2), (Some(id(102)), 2)]
    );
}

#[tokio::test]
async fn bounded_gap_survives_restart_without_skipping_and_new_arrivals_wait_for_next_sweep() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = WatchStateStore::open(dir.path(), Some(scope())).unwrap();
    baseline(&mut store, 100).await;

    let mut source = Source::pages(vec![vec![trigger(106), trigger(105)]]);
    let first = store
        .sweep(&mut source, &request(1, 2), &AtomicBool::new(false))
        .await
        .unwrap();
    assert!(first.gap);
    assert_eq!(first.checkpoint, Some(id(100)));
    assert_eq!(pending_ids(&store), [105, 106]);
    let resume = store.state().resume.as_ref().unwrap();
    assert_eq!(resume.captured_head, id(106));
    assert_eq!(resume.before, id(105));
    assert_eq!(resume.lower_checkpoint, Some(id(100)));
    drop(store);

    let mut store = WatchStateStore::open_existing(dir.path()).unwrap();
    // 109 arrives while the older bounded gap is being drained. A resume must
    // finish the captured 106 head before asking for the current newest page.
    let mut source = Source::pages(vec![
        vec![trigger(104), trigger(103)],
        vec![trigger(102), trigger(101)],
        vec![trigger(100), trigger(99)],
    ]);
    let resumed = store
        .sweep(&mut source, &request(3, 2), &AtomicBool::new(false))
        .await
        .unwrap();
    assert!(!resumed.gap);
    assert_eq!(resumed.checkpoint, Some(id(106)));
    assert_eq!(source.calls[0], (Some(id(105)), 2));
    assert_eq!(pending_ids(&store), [101, 102, 103, 104, 105, 106]);
    assert!(store.state().resume.is_none());

    let mut source = Source::pages(vec![vec![
        trigger(109),
        trigger(108),
        trigger(107),
        trigger(106),
    ]]);
    let next = store
        .sweep(&mut source, &request(1, 4), &AtomicBool::new(false))
        .await
        .unwrap();
    assert_eq!(next.checkpoint, Some(id(109)));
    assert_eq!(source.calls, [(None, 4)]);
    assert_eq!(
        pending_ids(&store),
        [101, 102, 103, 104, 105, 106, 107, 108, 109]
    );
}

#[tokio::test]
async fn failed_page_preserves_the_prior_committed_page_and_resume_cursor() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = WatchStateStore::open(dir.path(), Some(scope())).unwrap();
    baseline(&mut store, 100).await;
    let mut source = Source::pages(vec![vec![trigger(106), trigger(105)]]);
    source
        .pages
        .push_back(Err(eyre::eyre!("Synthetic page failure")));
    let _ = store
        .sweep(&mut source, &request(2, 2), &AtomicBool::new(false))
        .await
        .unwrap_err();
    assert_eq!(store.state().checkpoint, Some(id(100)));
    assert_eq!(pending_ids(&store), [105, 106]);
    assert_eq!(store.state().resume.as_ref().unwrap().before, id(105));
    drop(store);

    let store = WatchStateStore::open_existing(dir.path()).unwrap();
    assert_eq!(store.state().checkpoint, Some(id(100)));
    assert_eq!(pending_ids(&store), [105, 106]);
    assert_eq!(store.state().resume.as_ref().unwrap().before, id(105));
}

#[tokio::test]
async fn pending_replays_after_restart_and_acknowledgement_is_durable_and_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = WatchStateStore::open(dir.path(), Some(scope())).unwrap();
    baseline(&mut store, 100).await;
    let mut source = Source::pages(vec![vec![trigger(102), trigger(101), trigger(100)]]);
    store
        .sweep(&mut source, &request(1, 3), &AtomicBool::new(false))
        .await
        .unwrap();
    drop(store);

    let mut store = WatchStateStore::open_existing(dir.path()).unwrap();
    assert_eq!(pending_ids(&store), [101, 102]);
    let delivered = store
        .acknowledge(id(101), "synthetic-receipt-101".into())
        .unwrap();
    assert_eq!(
        delivered.delivered_receipt.as_deref(),
        Some("synthetic-receipt-101")
    );
    let repeated = store
        .acknowledge(id(101), "synthetic-receipt-101".into())
        .unwrap();
    assert_eq!(repeated.delivered_receipt, delivered.delivered_receipt);
    let snapshot_path = dir.path().join("watcher-state.json");
    let acknowledged = std::fs::read(&snapshot_path).unwrap();
    let _ = store.acknowledge(id(999), "unknown".into()).unwrap_err();
    let _ = store
        .acknowledge(id(101), "conflicting-receipt".into())
        .unwrap_err();
    for invalid in ["", " ", "line\nbreak"] {
        let _ = store.acknowledge(id(102), invalid.into()).unwrap_err();
    }
    assert_eq!(std::fs::read(snapshot_path).unwrap(), acknowledged);
    drop(store);

    let store = WatchStateStore::open_existing(dir.path()).unwrap();
    assert_eq!(pending_ids(&store), [102]);
    let delivered = store
        .state()
        .outbox
        .iter()
        .find(|entry| entry.message.id == id(101))
        .unwrap();
    assert_eq!(
        delivered.delivered_receipt.as_deref(),
        Some("synthetic-receipt-101")
    );
}

#[tokio::test]
async fn structured_trigger_filter_ignores_unrelated_author_bot_webhook_and_text_mentions() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = WatchStateStore::open(dir.path(), Some(scope())).unwrap();
    baseline(&mut store, 100).await;
    let mut other_author = trigger(107);
    other_author.author_id = WatcherUserId::new(31).unwrap();
    let mut bot = trigger(106);
    bot.author_is_bot = true;
    let mut webhook = trigger(105);
    webhook.is_webhook = true;
    let mut text_only = trigger(104);
    text_only.mentioned_user_ids.clear();
    text_only.content = "<@40> is only raw content".into();
    let mut different_mention = trigger(103);
    different_mention.mentioned_user_ids = vec![WatcherUserId::new(41).unwrap()];
    let mut unrelated = trigger(101);
    unrelated.author_id = WatcherUserId::new(31).unwrap();
    unrelated.mentioned_user_ids.clear();
    unrelated.reply_to = None;
    unrelated.reply = None;
    let mut source = Source::pages(vec![vec![
        other_author,
        bot,
        webhook,
        text_only,
        different_mention,
        trigger(102),
        unrelated,
        trigger(100),
    ]]);
    let report = store
        .sweep(&mut source, &request(1, 8), &AtomicBool::new(false))
        .await
        .unwrap();
    assert_eq!(report.queued, 1);
    assert_eq!(report.checkpoint, Some(id(107)));
    assert_eq!(pending_ids(&store), [102]);
}

#[tokio::test]
async fn malformed_qualifying_reply_rejects_the_whole_page_before_any_queue_commit() {
    for variant in 0..4 {
        let dir = tempfile::tempdir().unwrap();
        let mut store = WatchStateStore::open(dir.path(), Some(scope())).unwrap();
        baseline(&mut store, 100).await;
        let snapshot_path = dir.path().join("watcher-state.json");
        let original = std::fs::read(&snapshot_path).unwrap();
        let mut malformed = trigger(102);
        match variant {
            0 => malformed.reply = None,
            1 => malformed.reply.as_mut().unwrap().id = id(99),
            2 => malformed.reply.as_mut().unwrap().guild_id = WatcherGuildId::new(11).unwrap(),
            _ => malformed.reply.as_mut().unwrap().channel_id = WatcherChannelId::new(21).unwrap(),
        }
        let mut source = Source::pages(vec![vec![trigger(103), malformed, trigger(100)]]);
        let _ = store
            .sweep(&mut source, &request(1, 3), &AtomicBool::new(false))
            .await
            .unwrap_err();
        assert!(store.pending().is_empty());
        assert_eq!(store.state().checkpoint, Some(id(100)));
        assert_eq!(std::fs::read(snapshot_path).unwrap(), original);
    }
}

#[tokio::test]
async fn invalid_scope_or_pagination_is_rejected_without_partially_committing_a_page() {
    for variant in 0..5 {
        let dir = tempfile::tempdir().unwrap();
        let mut store = WatchStateStore::open(dir.path(), Some(scope())).unwrap();
        baseline(&mut store, 100).await;
        let snapshot_path = dir.path().join("watcher-state.json");
        let original = std::fs::read(&snapshot_path).unwrap();
        let mut page = vec![trigger(103), trigger(102)];
        match variant {
            0 => page[1].guild_id = WatcherGuildId::new(11).unwrap(),
            1 => page[1].channel_id = WatcherChannelId::new(21).unwrap(),
            2 => page[1].id = id(103),
            3 => page.reverse(),
            _ => page.push(trigger(101)),
        }
        let mut source = Source::pages(vec![page]);
        let _ = store
            .sweep(&mut source, &request(1, 2), &AtomicBool::new(false))
            .await
            .unwrap_err();
        assert_eq!(std::fs::read(snapshot_path).unwrap(), original);
        assert!(store.pending().is_empty());
    }
}

#[tokio::test]
async fn cancellation_after_response_discards_the_whole_page_and_pre_cancel_starts_no_request() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = WatchStateStore::open(dir.path(), Some(scope())).unwrap();
    baseline(&mut store, 100).await;
    let snapshot_path = dir.path().join("watcher-state.json");
    let original = std::fs::read(&snapshot_path).unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let mut source = Source::pages(vec![vec![trigger(103), trigger(102)]]);
    source.cancel_after_response = Some(Arc::clone(&cancel));
    let _ = store
        .sweep(&mut source, &request(1, 2), &cancel)
        .await
        .unwrap_err();
    assert_eq!(source.calls.len(), 1);
    assert_eq!(std::fs::read(&snapshot_path).unwrap(), original);
    assert!(store.pending().is_empty());
    let mut source = Source::pages(vec![]);
    let _ = store
        .sweep(&mut source, &request(1, 2), &cancel)
        .await
        .unwrap_err();
    assert!(source.calls.is_empty());
    assert_eq!(std::fs::read(snapshot_path).unwrap(), original);
}

#[tokio::test]
async fn corrupt_state_and_mismatched_scope_fail_without_overwriting_the_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = WatchStateStore::open(dir.path(), Some(scope())).unwrap();
    baseline(&mut store, 100).await;
    drop(store);
    let snapshot_path = dir.path().join("watcher-state.json");
    let original = std::fs::read(&snapshot_path).unwrap();
    let mut changed = scope();
    changed.author_id = WatcherUserId::new(31).unwrap();
    let _ = WatchStateStore::open(dir.path(), Some(changed)).unwrap_err();
    assert_eq!(std::fs::read(&snapshot_path).unwrap(), original);
    WatchStateStore::open_existing(dir.path()).unwrap();

    std::fs::write(&snapshot_path, b"{broken synthetic JSON").unwrap();
    let corrupt = std::fs::read(&snapshot_path).unwrap();
    let _ = WatchStateStore::open_existing(dir.path()).unwrap_err();
    assert_eq!(std::fs::read(snapshot_path).unwrap(), corrupt);
}

#[test]
fn a_second_store_cannot_take_the_lock_until_the_first_store_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let store = WatchStateStore::open(dir.path(), Some(scope())).unwrap();
    let _ = WatchStateStore::open(dir.path(), Some(scope())).unwrap_err();
    drop(store);
    let store = WatchStateStore::open(dir.path(), Some(scope())).unwrap();
    drop(store);
    assert!(dir.path().join("watcher-state.lock").exists());
}

const CHILD_STATE_DIR: &str = "TEAMY_DISCORD_WATCHER_TEST_STATE_DIR";

#[test]
#[ignore = "only invoked as a child by the cross-process lock test"]
fn watcher_lock_child() {
    let path = std::env::var_os(CHILD_STATE_DIR).expect("child state directory");
    let _store = WatchStateStore::open(std::path::Path::new(&path), Some(scope())).unwrap();
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(b"WATCHER_LOCK_READY\n").unwrap();
    stdout.flush().unwrap();
    let mut release = String::new();
    std::io::stdin().read_line(&mut release).unwrap();
}

struct LockChild(Child);
impl Drop for LockChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            match self.0.try_wait() {
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                _ => break,
            }
        }
    }
}

fn wait_for_child(child: &mut Child) -> ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "lock child did not exit within 10 seconds"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn lock_child(path: &std::path::Path) -> LockChild {
    let child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "watcher_lock_child", "--ignored", "--nocapture"])
        .env(CHILD_STATE_DIR, path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut child = LockChild(child);
    let stdout = child.0.stdout.take().unwrap();
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut reader = std::io::BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    let _ = sender.send(Err("lock child exited before readiness"));
                    return;
                }
                Ok(_) if line.trim() == "WATCHER_LOCK_READY" => {
                    let _ = sender.send(Ok(reader.into_inner()));
                    return;
                }
                Ok(_) => {}
                Err(_) => {
                    let _ = sender.send(Err("lock child readiness read failed"));
                    return;
                }
            }
        }
    });
    child.0.stdout = Some(
        receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("lock child did not become ready within 10 seconds")
            .expect("lock child failed before readiness"),
    );
    child
}

#[test]
fn cross_process_contention_and_process_exit_release_the_os_lock() {
    for forced in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let mut child = lock_child(dir.path());
        let _ = WatchStateStore::open(dir.path(), Some(scope())).unwrap_err();
        if forced {
            child.0.kill().unwrap();
            let _ = wait_for_child(&mut child.0);
        } else {
            child
                .0
                .stdin
                .as_mut()
                .unwrap()
                .write_all(b"release\n")
                .unwrap();
            assert!(wait_for_child(&mut child.0).success());
        }
        let store = WatchStateStore::open(dir.path(), Some(scope())).unwrap();
        drop(store);
        assert!(dir.path().join("watcher-state.lock").exists());
    }
}

#[tokio::test]
async fn invalid_settings_and_repeated_backfill_fail_before_a_source_request() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = WatchStateStore::open(dir.path(), Some(scope())).unwrap();
    baseline(&mut store, 100).await;
    let snapshot_path = dir.path().join("watcher-state.json");
    let original = std::fs::read(&snapshot_path).unwrap();
    for invalid in [
        request(0, 2),
        request(1001, 2),
        request(1, 0),
        request(1, 101),
    ] {
        let mut source = Source::pages(vec![]);
        let error = store
            .sweep(&mut source, &invalid, &AtomicBool::new(false))
            .await
            .unwrap_err();
        assert_eq!(
            error.downcast_ref::<teamy_discord::watcher::WatcherError>(),
            Some(&teamy_discord::watcher::WatcherError::InvalidSettings)
        );
        assert!(source.calls.is_empty());
        assert_eq!(std::fs::read(&snapshot_path).unwrap(), original);
    }
    let mut repeated = request(1, 2);
    repeated.backfill_after_id = Some(id(99));
    let mut source = Source::pages(vec![]);
    let error = store
        .sweep(&mut source, &repeated, &AtomicBool::new(false))
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<teamy_discord::watcher::WatcherError>(),
        Some(&teamy_discord::watcher::WatcherError::AlreadyInitialized)
    );
    assert!(source.calls.is_empty());
    assert_eq!(std::fs::read(snapshot_path).unwrap(), original);
}

#[tokio::test]
async fn forged_persisted_outbox_and_unsupported_schema_are_rejected_without_resetting_state() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = WatchStateStore::open(dir.path(), Some(scope())).unwrap();
    baseline(&mut store, 100).await;
    let mut source = Source::pages(vec![vec![trigger(102), trigger(100)]]);
    store
        .sweep(&mut source, &request(1, 2), &AtomicBool::new(false))
        .await
        .unwrap();
    let original = store.state().clone();
    drop(store);
    let snapshot_path = dir.path().join("watcher-state.json");
    for variant in 0..9 {
        let mut forged = original.clone();
        match variant {
            0 => forged.outbox[0].message.author_id = WatcherUserId::new(31).unwrap(),
            1 => forged.outbox[0].message.author_is_bot = true,
            2 => forged.outbox[0].message.is_webhook = true,
            3 => forged.outbox[0].message.mentioned_user_ids.clear(),
            4 => forged.outbox[0].message.reply.as_mut().unwrap().id = id(99),
            5 => forged.outbox[0].message.guild_id = WatcherGuildId::new(11).unwrap(),
            6 => forged.schema_version = 999,
            7 => forged.outbox[0].message.id = id(999),
            _ => forged.outbox.push(forged.outbox[0].clone()),
        }
        let encoded = facet_json::to_string_pretty(&forged).unwrap();
        std::fs::write(&snapshot_path, encoded.as_bytes()).unwrap();
        let error = WatchStateStore::open_existing(dir.path()).unwrap_err();
        assert_eq!(
            error.downcast_ref::<teamy_discord::watcher::WatcherError>(),
            Some(&teamy_discord::watcher::WatcherError::CorruptState)
        );
        assert_eq!(std::fs::read(&snapshot_path).unwrap(), encoded.as_bytes());
    }
}

#[tokio::test]
async fn stale_pending_is_not_committed_and_staging_failure_preserves_the_previous_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = WatchStateStore::open(dir.path(), Some(scope())).unwrap();
    baseline(&mut store, 100).await;
    drop(store);
    let pending_path = dir.path().join("watcher-state.json.pending");
    std::fs::write(&pending_path, b"incomplete synthetic staging").unwrap();
    let mut store = WatchStateStore::open_existing(dir.path()).unwrap();
    assert_eq!(store.state().checkpoint, Some(id(100)));
    assert!(store.pending().is_empty());
    let mut source = Source::pages(vec![vec![trigger(102), trigger(101), trigger(100)]]);
    store
        .sweep(&mut source, &request(1, 3), &AtomicBool::new(false))
        .await
        .unwrap();
    assert_eq!(pending_ids(&store), [101, 102]);
    assert!(!pending_path.exists());

    let snapshot_path = dir.path().join("watcher-state.json");
    let original = std::fs::read(&snapshot_path).unwrap();
    // A directory occupying this synthetic staging path deterministically fails
    // file creation on both Windows and Unix, without relying on ACL changes.
    std::fs::create_dir(&pending_path).unwrap();
    let mut source = Source::pages(vec![vec![trigger(104), trigger(103), trigger(102)]]);
    let error = store
        .sweep(&mut source, &request(1, 3), &AtomicBool::new(false))
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<teamy_discord::watcher::WatcherError>(),
        Some(&teamy_discord::watcher::WatcherError::Persistence)
    );
    assert_eq!(store.state().checkpoint, Some(id(102)));
    assert_eq!(pending_ids(&store), [101, 102]);
    assert_eq!(std::fs::read(&snapshot_path).unwrap(), original);
    drop(store);
    let store = WatchStateStore::open_existing(dir.path()).unwrap();
    assert_eq!(pending_ids(&store), [101, 102]);
}

#[tokio::test]
async fn nonadvancing_resume_page_cannot_erase_the_prior_durable_gap() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = WatchStateStore::open(dir.path(), Some(scope())).unwrap();
    baseline(&mut store, 100).await;
    let mut source = Source::pages(vec![vec![trigger(106), trigger(105)]]);
    store
        .sweep(&mut source, &request(1, 2), &AtomicBool::new(false))
        .await
        .unwrap();
    let snapshot_path = dir.path().join("watcher-state.json");
    let original = std::fs::read(&snapshot_path).unwrap();
    let mut source = Source::pages(vec![vec![trigger(105), trigger(104)]]);
    let error = store
        .sweep(&mut source, &request(1, 2), &AtomicBool::new(false))
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<teamy_discord::watcher::WatcherError>(),
        Some(&teamy_discord::watcher::WatcherError::InvalidPage)
    );
    assert_eq!(source.calls, [(Some(id(105)), 2)]);
    assert_eq!(store.state().checkpoint, Some(id(100)));
    assert_eq!(store.state().resume.as_ref().unwrap().before, id(105));
    assert_eq!(pending_ids(&store), [105, 106]);
    assert_eq!(std::fs::read(snapshot_path).unwrap(), original);
}
