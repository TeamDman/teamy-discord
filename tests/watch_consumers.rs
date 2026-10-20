use std::io::Write as _;
use std::path::Path;
use std::process::Child;
use std::process::Command;
use std::process::ExitStatus;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;
use teamy_discord::watch_consumers::ConsumerClaimResult;
use teamy_discord::watch_consumers::ConsumerDelivery;
use teamy_discord::watch_consumers::ConsumerDeliveryId;
use teamy_discord::watch_consumers::ConsumerError;
use teamy_discord::watch_consumers::ConsumerName;
use teamy_discord::watch_consumers::ConsumerRequestBinding;
use teamy_discord::watch_consumers::ConsumerRequestId;
use teamy_discord::watch_consumers::ConsumerStart;
use teamy_discord::watch_consumers::ConsumerStore;
use teamy_discord::watcher::WatcherChannelId;
use teamy_discord::watcher::WatcherGuildId;
use teamy_discord::watcher::WatcherMessage;
use teamy_discord::watcher::WatcherMessageId;
use teamy_discord::watcher::WatcherReply;
use teamy_discord::watcher::WatcherScope;
use teamy_discord::watcher::WatcherUserId;

fn event_id(value: u64) -> WatcherMessageId {
    WatcherMessageId::new(value).unwrap()
}

fn name(value: &str) -> ConsumerName {
    ConsumerName::new(value).unwrap()
}

fn request(value: u64) -> ConsumerRequestId {
    ConsumerRequestId::new(format!("00000000-0000-4000-8000-{value:012x}")).unwrap()
}

fn scope() -> WatcherScope {
    WatcherScope {
        guild_id: WatcherGuildId::new(10).unwrap(),
        channel_id: WatcherChannelId::new(20).unwrap(),
        author_id: WatcherUserId::new(30).unwrap(),
        bot_id: WatcherUserId::new(40).unwrap(),
    }
}

fn message(value: u64) -> WatcherMessage {
    WatcherMessage {
        id: event_id(value),
        guild_id: WatcherGuildId::new(10).unwrap(),
        channel_id: WatcherChannelId::new(20).unwrap(),
        author_id: WatcherUserId::new(30).unwrap(),
        author_is_bot: false,
        is_webhook: false,
        mentioned_user_ids: vec![WatcherUserId::new(40).unwrap()],
        content: format!("Synthetic concern {value}"),
        reply_to: Some(event_id(value - 1)),
        reply: Some(WatcherReply {
            id: event_id(value - 1),
            guild_id: WatcherGuildId::new(10).unwrap(),
            channel_id: WatcherChannelId::new(20).unwrap(),
            author_id: WatcherUserId::new(50).unwrap(),
            content: "Synthetic referenced message".into(),
        }),
    }
}

fn claimed(result: ConsumerClaimResult) -> ConsumerDelivery {
    match result {
        ConsumerClaimResult::Claimed(delivery) => delivery,
        _ => panic!("Expected a new claimed payload"),
    }
}

fn snapshot(directory: &Path) -> Vec<u8> {
    std::fs::read(directory.join("consumers-state.json")).unwrap()
}

fn expect_error(error: &eyre::Report, expected: ConsumerError) {
    assert_eq!(error.downcast_ref::<ConsumerError>(), Some(&expected));
}

#[test]
fn typed_identifiers_validate_boundaries_and_normalize_request_ids() {
    for valid in ["a", "A1", "worker_2-test", &"a".repeat(64)] {
        let parsed = ConsumerName::new(valid).unwrap();
        assert_eq!(parsed.as_str(), valid);
        assert_eq!(valid.parse::<ConsumerName>().unwrap().to_string(), valid);
    }
    for invalid in [
        "",
        "-first",
        "_first",
        "contains space",
        "a/b",
        "é",
        &"a".repeat(65),
    ] {
        let _ = ConsumerName::new(invalid).unwrap_err();
    }
    let canonical = request(1).to_string();
    assert_eq!(
        canonical.parse::<ConsumerRequestId>().unwrap().as_str(),
        canonical
    );
    assert_eq!(
        ConsumerRequestId::new("00000000-0000-4000-8000-00000000000A")
            .unwrap()
            .as_str(),
        "00000000-0000-4000-8000-00000000000a"
    );
    for invalid in [
        "",
        "not-a-uuid",
        "00000000000040008000000000000001",
        "00000000-0000-0000-0000-000000000000",
    ] {
        let _ = ConsumerRequestId::new(invalid).unwrap_err();
    }
    let delivery = format!("d1-{}", "a".repeat(64));
    assert_eq!(
        delivery.parse::<ConsumerDeliveryId>().unwrap().as_str(),
        delivery
    );
    for invalid in [
        "",
        "d1-abcd",
        &format!("d1-{}", "A".repeat(64)),
        &format!("d2-{}", "a".repeat(64)),
    ] {
        let _ = ConsumerDeliveryId::new(invalid).unwrap_err();
    }
}

#[test]
fn append_sequence_is_sorted_within_batch_and_delayed_older_events_append_after_existing_history() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ConsumerStore::open(dir.path(), scope()).unwrap();
    let mut duplicate = message(101);
    duplicate.content = "Changed duplicate must not replace original".into();
    let published = store
        .publish(vec![message(103), message(101), message(102), duplicate])
        .unwrap();
    assert_eq!(published.appended, 3);
    assert_eq!(published.last_sequence, 3);
    let worker = name("worker");
    store.create(worker.clone(), ConsumerStart::Oldest).unwrap();
    for (sequence, id) in [(1, 101), (2, 102), (3, 103)] {
        let delivery = claimed(store.claim(&worker, request(sequence)).unwrap());
        assert_eq!(delivery.claim.event_sequence, sequence);
        assert_eq!(delivery.claim.event_id, event_id(id));
        assert_eq!(delivery.message.content, format!("Synthetic concern {id}"));
        store
            .ack(
                &worker,
                event_id(id),
                &delivery.claim.delivery_id,
                format!("receipt-{id}"),
            )
            .unwrap();
    }

    let mut changed = message(101);
    changed.content = "Changed after initial batch".into();
    let published = store.publish(vec![changed, message(99)]).unwrap();
    assert_eq!(published.appended, 1);
    assert_eq!(published.last_sequence, 4);
    let delayed = claimed(store.claim(&worker, request(4)).unwrap());
    assert_eq!(delayed.claim.event_id, event_id(99));
    assert_eq!(delayed.claim.event_sequence, 4);
    assert_eq!(
        store.state().events[0].message.content,
        "Synthetic concern 101"
    );
    drop(store);

    let store = ConsumerStore::open(dir.path(), scope()).unwrap();
    assert_eq!(store.state().events.len(), 4);
    assert_eq!(
        store.state().events[0].message.content,
        "Synthetic concern 101"
    );
    assert_eq!(store.state().events[3].message.id, event_id(99));
}

#[test]
fn now_and_oldest_have_independent_start_positions_and_reusing_a_name_never_resets_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ConsumerStore::open(dir.path(), scope()).unwrap();
    store.publish(vec![message(101), message(102)]).unwrap();
    let historical = name("history");
    let fresh = name("fresh");
    assert_eq!(
        store
            .create(historical.clone(), ConsumerStart::Oldest)
            .unwrap()
            .cursor_sequence,
        0
    );
    assert_eq!(
        store
            .create(fresh.clone(), ConsumerStart::Now)
            .unwrap()
            .cursor_sequence,
        2
    );
    assert!(matches!(
        store.claim(&fresh, request(1)).unwrap(),
        ConsumerClaimResult::Empty
    ));

    let old = claimed(store.claim(&historical, request(2)).unwrap());
    let reused = store
        .create(historical.clone(), ConsumerStart::Now)
        .unwrap();
    assert_eq!(reused.cursor_sequence, 0);
    assert_eq!(reused.in_flight.unwrap().delivery_id, old.claim.delivery_id);
    store
        .ack(
            &historical,
            event_id(101),
            &old.claim.delivery_id,
            "history-101".into(),
        )
        .unwrap();
    assert_eq!(
        store
            .create(historical.clone(), ConsumerStart::Oldest)
            .unwrap()
            .cursor_sequence,
        1
    );

    store.publish(vec![message(103)]).unwrap();
    let fresh_delivery = claimed(store.claim(&fresh, request(3)).unwrap());
    assert_eq!(fresh_delivery.claim.event_id, event_id(103));
    let next_old = claimed(store.claim(&historical, request(4)).unwrap());
    assert_eq!(next_old.claim.event_id, event_id(102));
}

#[test]
fn two_consumers_have_separate_cursors_claims_and_delivery_identities() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ConsumerStore::open(dir.path(), scope()).unwrap();
    store.publish(vec![message(101), message(102)]).unwrap();
    let alpha = name("alpha");
    let beta = name("beta");
    store.create(alpha.clone(), ConsumerStart::Oldest).unwrap();
    store.create(beta.clone(), ConsumerStart::Oldest).unwrap();
    let first_alpha = claimed(store.claim(&alpha, request(1)).unwrap());
    let first_beta = claimed(store.claim(&beta, request(2)).unwrap());
    assert_eq!(first_alpha.claim.event_id, first_beta.claim.event_id);
    assert_ne!(first_alpha.claim.delivery_id, first_beta.claim.delivery_id);
    let _ = store
        .ack(
            &alpha,
            event_id(101),
            &first_beta.claim.delivery_id,
            "wrong-consumer".into(),
        )
        .unwrap_err();
    store
        .ack(
            &alpha,
            event_id(101),
            &first_alpha.claim.delivery_id,
            "alpha-101".into(),
        )
        .unwrap();
    let second_alpha = claimed(store.claim(&alpha, request(3)).unwrap());
    assert_eq!(second_alpha.claim.event_id, event_id(102));
    assert_eq!(store.inspect(&alpha).unwrap().cursor_sequence, 1);
    assert_eq!(store.inspect(&beta).unwrap().cursor_sequence, 0);
    drop(store);

    let store = ConsumerStore::open(dir.path(), scope()).unwrap();
    assert_eq!(
        store.inspect(&alpha).unwrap().in_flight.unwrap().event_id,
        event_id(102)
    );
    assert_eq!(
        store.inspect(&beta).unwrap().in_flight.unwrap().delivery_id,
        first_beta.claim.delivery_id
    );
}

#[test]
fn lost_claim_payload_retries_only_metadata_until_explicit_current_delivery_replay() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ConsumerStore::open(dir.path(), scope()).unwrap();
    store.publish(vec![message(101), message(102)]).unwrap();
    let worker = name("worker");
    store.create(worker.clone(), ConsumerStart::Oldest).unwrap();
    let original = claimed(store.claim(&worker, request(1)).unwrap());
    drop(store);

    let mut store = ConsumerStore::open(dir.path(), scope()).unwrap();
    for request_id in [1, 2, 1, 2] {
        match store.claim(&worker, request(request_id)).unwrap() {
            ConsumerClaimResult::PendingAck(claim) => {
                assert_eq!(claim.delivery_id, original.claim.delivery_id);
                assert_eq!(claim.event_id, event_id(101));
            }
            _ => panic!("A pending durable claim must not automatically resend a payload"),
        }
    }
    let replay = store.replay(&worker, &original.claim.delivery_id).unwrap();
    assert_eq!(replay.message, original.message);
    assert_eq!(replay.claim.delivery_id, original.claim.delivery_id);
    let other_delivery = ConsumerDeliveryId::new(format!("d1-{}", "f".repeat(64))).unwrap();
    let _ = store.replay(&worker, &other_delivery).unwrap_err();
    assert_eq!(store.inspect(&worker).unwrap().cursor_sequence, 0);
    assert_eq!(
        store.inspect(&worker).unwrap().in_flight.unwrap().event_id,
        event_id(101)
    );
}

#[test]
fn completed_request_retries_win_over_a_newer_live_claim_and_lost_ack_reply_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ConsumerStore::open(dir.path(), scope()).unwrap();
    store.publish(vec![message(101), message(102)]).unwrap();
    let worker = name("worker");
    store.create(worker.clone(), ConsumerStart::Oldest).unwrap();
    let first = claimed(store.claim(&worker, request(1)).unwrap());
    assert!(matches!(
        store.claim(&worker, request(2)).unwrap(),
        ConsumerClaimResult::PendingAck(_)
    ));
    let first_ack = store
        .ack(
            &worker,
            event_id(101),
            &first.claim.delivery_id,
            "receipt-101".into(),
        )
        .unwrap();
    let second = claimed(store.claim(&worker, request(3)).unwrap());
    assert_eq!(second.claim.event_id, event_id(102));
    drop(store);

    let mut store = ConsumerStore::open(dir.path(), scope()).unwrap();
    for request_id in [1, 2] {
        match store.claim(&worker, request(request_id)).unwrap() {
            ConsumerClaimResult::AlreadyCompleted(ack) => {
                assert_eq!(ack.event_id, event_id(101));
                assert_eq!(ack.delivery_id, first_ack.delivery_id);
                assert_eq!(ack.receipt, "receipt-101");
            }
            _ => panic!("A completed request must not borrow a newer in-flight claim"),
        }
    }
    let retried_ack = store
        .ack(
            &worker,
            event_id(101),
            &first.claim.delivery_id,
            "receipt-101".into(),
        )
        .unwrap();
    assert_eq!(retried_ack.delivery_id, first_ack.delivery_id);
    let before_conflict = snapshot(dir.path());
    let _ = store
        .ack(
            &worker,
            event_id(101),
            &first.claim.delivery_id,
            "conflicting-receipt".into(),
        )
        .unwrap_err();
    assert_eq!(snapshot(dir.path()), before_conflict);
    assert_eq!(
        store
            .inspect(&worker)
            .unwrap()
            .in_flight
            .unwrap()
            .delivery_id,
        second.claim.delivery_id
    );
    assert_eq!(store.inspect(&worker).unwrap().cursor_sequence, 1);
    let _ = store.replay(&worker, &first.claim.delivery_id).unwrap_err();
    let replay = store.replay(&worker, &second.claim.delivery_id).unwrap();
    assert_eq!(replay.message.id, event_id(102));
}

#[test]
fn acknowledgments_reject_wrong_identity_or_receipt_without_changing_a_live_claim() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ConsumerStore::open(dir.path(), scope()).unwrap();
    store.publish(vec![message(101)]).unwrap();
    let worker = name("worker");
    store.create(worker.clone(), ConsumerStart::Oldest).unwrap();
    let delivery = claimed(store.claim(&worker, request(1)).unwrap());
    let original = snapshot(dir.path());
    let other_delivery = ConsumerDeliveryId::new(format!("d1-{}", "f".repeat(64))).unwrap();
    let _ = store
        .ack(
            &worker,
            event_id(102),
            &delivery.claim.delivery_id,
            "wrong-event".into(),
        )
        .unwrap_err();
    let _ = store
        .ack(
            &worker,
            event_id(101),
            &other_delivery,
            "wrong-delivery".into(),
        )
        .unwrap_err();
    for invalid in ["", " ", "line\nbreak", &"r".repeat(4097)] {
        let error = store
            .ack(
                &worker,
                event_id(101),
                &delivery.claim.delivery_id,
                invalid.into(),
            )
            .unwrap_err();
        expect_error(&error, ConsumerError::InvalidReceipt);
    }
    assert_eq!(snapshot(dir.path()), original);
    assert_eq!(
        store
            .inspect(&worker)
            .unwrap()
            .in_flight
            .unwrap()
            .delivery_id,
        delivery.claim.delivery_id
    );
    assert_eq!(store.inspect(&worker).unwrap().cursor_sequence, 0);
}

#[test]
fn a_request_id_cannot_change_operation_consumer_or_typed_parameters() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ConsumerStore::open(dir.path(), scope()).unwrap();
    let worker = name("worker");
    let other = name("other");
    store.create(worker.clone(), ConsumerStart::Oldest).unwrap();
    let first_delivery = ConsumerDeliveryId::new(format!("d1-{}", "a".repeat(64))).unwrap();
    let second_delivery = ConsumerDeliveryId::new(format!("d1-{}", "b".repeat(64))).unwrap();
    let cases = [
        (
            ConsumerRequestBinding::create(worker.clone(), ConsumerStart::Oldest),
            ConsumerRequestBinding::create(worker.clone(), ConsumerStart::Now),
        ),
        (
            ConsumerRequestBinding::claim(worker.clone()),
            ConsumerRequestBinding::inspect(worker.clone()),
        ),
        (
            ConsumerRequestBinding::inspect(worker.clone()),
            ConsumerRequestBinding::inspect(other.clone()),
        ),
        (
            ConsumerRequestBinding::replay(worker.clone(), first_delivery.clone()),
            ConsumerRequestBinding::replay(worker.clone(), second_delivery.clone()),
        ),
        (
            ConsumerRequestBinding::ack(
                worker.clone(),
                event_id(101),
                first_delivery.clone(),
                "receipt".into(),
            ),
            ConsumerRequestBinding::ack(
                worker.clone(),
                event_id(102),
                first_delivery.clone(),
                "receipt".into(),
            ),
        ),
        (
            ConsumerRequestBinding::ack(
                worker.clone(),
                event_id(101),
                first_delivery.clone(),
                "receipt".into(),
            ),
            ConsumerRequestBinding::ack(
                worker.clone(),
                event_id(101),
                second_delivery,
                "receipt".into(),
            ),
        ),
        (
            ConsumerRequestBinding::ack(
                worker.clone(),
                event_id(101),
                first_delivery.clone(),
                "receipt".into(),
            ),
            ConsumerRequestBinding::ack(
                worker.clone(),
                event_id(101),
                first_delivery,
                "changed".into(),
            ),
        ),
    ];
    for (index, (original, conflicting)) in cases.into_iter().enumerate() {
        let request_id = request(u64::try_from(index).unwrap() + 1);
        store
            .bind_request(request_id.clone(), original.clone())
            .unwrap();
        store.bind_request(request_id.clone(), original).unwrap();
        let before = snapshot(dir.path());
        let error = store.bind_request(request_id, conflicting).unwrap_err();
        expect_error(&error, ConsumerError::RequestConflict);
        assert_eq!(snapshot(dir.path()), before);
    }
    drop(store);

    let mut store = ConsumerStore::open(dir.path(), scope()).unwrap();
    let error = store.claim(&worker, request(1)).unwrap_err();
    expect_error(&error, ConsumerError::RequestConflict);
}

#[test]
fn publishing_any_unauthorized_or_malformed_message_rejects_the_entire_batch() {
    for variant in 0..7 {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ConsumerStore::open(dir.path(), scope()).unwrap();
        store.publish(vec![message(101)]).unwrap();
        let original = snapshot(dir.path());
        let mut invalid = message(103);
        match variant {
            0 => invalid.author_id = WatcherUserId::new(31).unwrap(),
            1 => invalid.author_is_bot = true,
            2 => invalid.is_webhook = true,
            3 => invalid.mentioned_user_ids.clear(),
            4 => invalid.guild_id = WatcherGuildId::new(11).unwrap(),
            5 => invalid.channel_id = WatcherChannelId::new(21).unwrap(),
            _ => invalid.reply.as_mut().unwrap().id = event_id(99),
        }
        let _ = store.publish(vec![message(102), invalid]).unwrap_err();
        assert_eq!(snapshot(dir.path()), original);
        assert_eq!(store.state().events.len(), 1);
    }
}

#[test]
fn unknown_consumers_are_not_implicitly_created() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ConsumerStore::open(dir.path(), scope()).unwrap();
    store.publish(vec![message(101)]).unwrap();
    let absent = name("absent");
    let original = snapshot(dir.path());
    let error = store.inspect(&absent).unwrap_err();
    expect_error(&error, ConsumerError::UnknownConsumer);
    let error = store.claim(&absent, request(1)).unwrap_err();
    expect_error(&error, ConsumerError::UnknownConsumer);
    assert_eq!(snapshot(dir.path()), original);
    assert!(store.state().consumers.is_empty());
}

#[test]
fn scope_mismatch_corrupt_json_and_unsupported_schema_fail_without_resetting_the_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ConsumerStore::open(dir.path(), scope()).unwrap();
    store.publish(vec![message(101)]).unwrap();
    let original_state = store.state().clone();
    drop(store);
    let original = snapshot(dir.path());
    let mut changed_scope = scope();
    changed_scope.author_id = WatcherUserId::new(31).unwrap();
    let error = ConsumerStore::open(dir.path(), changed_scope).unwrap_err();
    expect_error(&error, ConsumerError::ScopeMismatch);
    assert_eq!(snapshot(dir.path()), original);

    let mut unsupported = original_state;
    unsupported.schema_version = 999;
    let encoded = facet_json::to_string_pretty(&unsupported).unwrap();
    std::fs::write(dir.path().join("consumers-state.json"), encoded.as_bytes()).unwrap();
    let error = ConsumerStore::open(dir.path(), scope()).unwrap_err();
    expect_error(&error, ConsumerError::CorruptState);
    assert_eq!(snapshot(dir.path()), encoded.as_bytes());

    std::fs::write(
        dir.path().join("consumers-state.json"),
        b"{synthetic partial JSON",
    )
    .unwrap();
    let corrupt = snapshot(dir.path());
    let error = ConsumerStore::open(dir.path(), scope()).unwrap_err();
    expect_error(&error, ConsumerError::CorruptState);
    assert_eq!(snapshot(dir.path()), corrupt);
}

#[test]
fn stale_pending_is_ignored_and_failed_staging_cannot_advance_a_consumer_or_append_events() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ConsumerStore::open(dir.path(), scope()).unwrap();
    store.publish(vec![message(101)]).unwrap();
    let worker = name("worker");
    store.create(worker.clone(), ConsumerStart::Oldest).unwrap();
    drop(store);
    let pending_path = dir.path().join("consumers-state.json.pending");
    std::fs::write(&pending_path, b"incomplete synthetic staging").unwrap();
    let mut store = ConsumerStore::open(dir.path(), scope()).unwrap();
    assert_eq!(store.state().events.len(), 1);
    assert_eq!(store.inspect(&worker).unwrap().cursor_sequence, 0);
    store.publish(vec![message(102)]).unwrap();
    assert!(!pending_path.exists());

    std::fs::create_dir(&pending_path).unwrap();
    let original = snapshot(dir.path());
    let error = store.publish(vec![message(103)]).unwrap_err();
    expect_error(&error, ConsumerError::Persistence);
    assert_eq!(store.state().events.len(), 2);
    assert_eq!(snapshot(dir.path()), original);
    let error = store.claim(&worker, request(1)).unwrap_err();
    expect_error(&error, ConsumerError::Persistence);
    assert!(store.inspect(&worker).unwrap().in_flight.is_none());
    assert_eq!(snapshot(dir.path()), original);
    drop(store);

    let store = ConsumerStore::open(dir.path(), scope()).unwrap();
    assert_eq!(store.state().events.len(), 2);
    assert_eq!(store.inspect(&worker).unwrap().cursor_sequence, 0);
    assert!(store.inspect(&worker).unwrap().in_flight.is_none());
}

#[test]
fn a_live_store_excludes_competing_handles_and_drop_releases_the_permanent_os_lock() {
    let dir = tempfile::tempdir().unwrap();
    let store = ConsumerStore::open(dir.path(), scope()).unwrap();
    let error = ConsumerStore::open(dir.path(), scope()).unwrap_err();
    expect_error(&error, ConsumerError::StateBusy);
    drop(store);
    let store = ConsumerStore::open(dir.path(), scope()).unwrap();
    drop(store);
    assert!(dir.path().join("consumers-state.lock").exists());
}

const CHILD_DIRECTORY: &str = "TEAMY_DISCORD_CONSUMER_TEST_DIRECTORY";

#[test]
#[ignore = "invoked only by the bounded subprocess recovery test"]
fn consumer_process_child() {
    let directory = std::env::var_os(CHILD_DIRECTORY).expect("synthetic child directory");
    let directory = Path::new(&directory);
    let mut store = ConsumerStore::open(directory, scope()).unwrap();
    store.publish(vec![message(101), message(102)]).unwrap();
    let worker = name("worker");
    store.create(worker.clone(), ConsumerStart::Oldest).unwrap();
    let _delivery = claimed(store.claim(&worker, request(1)).unwrap());
    // This marker is emitted after claim persistence, without publishing its payload.
    std::fs::write(directory.join("synthetic-child-ready"), b"ready").unwrap();
    let mut release = String::new();
    std::io::stdin().read_line(&mut release).unwrap();
}

struct ProcessChild(Child);
impl Drop for ProcessChild {
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
            "synthetic child did not exit within 10 seconds"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn process_exit_releases_the_lock_and_a_crash_after_claim_commit_requires_explicit_replay() {
    for forced in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "consumer_process_child",
                "--ignored",
                "--nocapture",
            ])
            .env(CHILD_DIRECTORY, dir.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut child = ProcessChild(child);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !dir.path().join("synthetic-child-ready").exists() {
            assert!(
                child.0.try_wait().unwrap().is_none(),
                "synthetic child exited before claim readiness"
            );
            assert!(
                Instant::now() < deadline,
                "synthetic child did not commit its claim within 10 seconds"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        let error = ConsumerStore::open(dir.path(), scope()).unwrap_err();
        expect_error(&error, ConsumerError::StateBusy);
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

        let mut store = ConsumerStore::open(dir.path(), scope()).unwrap();
        let worker = name("worker");
        let in_flight = store.inspect(&worker).unwrap().in_flight.unwrap();
        assert_eq!(in_flight.event_id, event_id(101));
        assert_eq!(store.inspect(&worker).unwrap().cursor_sequence, 0);
        assert!(matches!(
            store.claim(&worker, request(1)).unwrap(),
            ConsumerClaimResult::PendingAck(_)
        ));
        let replay = store.replay(&worker, &in_flight.delivery_id).unwrap();
        assert_eq!(replay.message.id, event_id(101));
        assert_eq!(replay.message.content, "Synthetic concern 101");
        assert!(dir.path().join("consumers-state.lock").exists());
    }
}

#[test]
fn completed_claims_reject_renamed_or_missing_origin_requests_without_resetting_state() {
    for remove_origin_with_alias_retained in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ConsumerStore::open(dir.path(), scope()).unwrap();
        store.publish(vec![message(101), message(102)]).unwrap();
        let worker = name("worker");
        store.create(worker.clone(), ConsumerStart::Oldest).unwrap();
        let first = claimed(store.claim(&worker, request(1)).unwrap());
        if remove_origin_with_alias_retained {
            assert!(matches!(
                store.claim(&worker, request(2)).unwrap(),
                ConsumerClaimResult::PendingAck(_)
            ));
        }
        store
            .ack(
                &worker,
                event_id(101),
                &first.claim.delivery_id,
                "receipt-101".into(),
            )
            .unwrap();
        let status = store.inspect(&worker).unwrap();
        assert_eq!(status.cursor_sequence, 1);
        assert_eq!(status.last_published_sequence, 2);
        assert!(status.in_flight.is_none());
        let mut corrupt = store.state().clone();
        drop(store);

        if remove_origin_with_alias_retained {
            corrupt.requests.retain(|record| record.id != request(1));
            assert_eq!(corrupt.requests.len(), 1);
            assert_eq!(corrupt.requests[0].id, request(2));
        } else {
            let origin = corrupt
                .requests
                .iter_mut()
                .find(|record| record.id == request(1))
                .unwrap();
            origin.id = request(2);
        }
        // The completed claim still identifies request 1 as its origin. If this
        // snapshot were accepted, retrying that request could newly claim event 102.
        assert_eq!(
            corrupt.requests[0].claim.as_ref().unwrap().request_id,
            request(1)
        );
        assert!(corrupt.requests[0].completed_ack.is_some());
        let encoded = facet_json::to_string_pretty(&corrupt).unwrap();
        std::fs::write(dir.path().join("consumers-state.json"), encoded.as_bytes()).unwrap();
        let error = ConsumerStore::open(dir.path(), scope()).unwrap_err();
        expect_error(&error, ConsumerError::CorruptState);
        assert_eq!(snapshot(dir.path()), encoded.as_bytes());
    }
}
