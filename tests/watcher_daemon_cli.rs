//! CLI-only qualification: plans, help and framing never start a capture service or contact Discord.
use std::io::Read;
use std::io::Write;
use std::io::{self};
use std::path::Path;
use std::process::Command;
use std::process::Output;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;
use teamy_discord::cli::watch::WatchClientPlan;
use teamy_discord::cli::watch::render_response::render_response;
use teamy_discord::watch_consumers::ConsumerName;
use teamy_discord::watch_consumers::ConsumerRequestId;
use teamy_discord::watch_consumers::ConsumerStart;
use teamy_discord::watch_daemon::DaemonCommand;
use teamy_discord::watch_daemon::DaemonRequest;
use teamy_discord::watch_daemon::PROTOCOL_VERSION;

const REQUEST_ID: &str = "b9b28218-cb82-4a5f-a72b-4b123d332d01";
const DELIVERY_ID: &str = "d1-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn run(home: &Path, args: &[&str]) -> Output {
    run_with_filter(home, args, "off")
}

fn run_with_filter(home: &Path, args: &[&str], filter: &str) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_teamy-discord"));
    for name in [
        "TEAMY_DISCORD_BOT_TOKEN",
        "TEAMY_DISCORD_ARCHIVE_DISCORD_BOT_TOKEN",
        "TEAMY_DISCORD_BROKER_ACCOUNT",
        "TEAMY_DISCORD_BROKER_SECRET_REF",
        "TEAMY_DISCORD_CREDENTIAL_PROVIDER",
        "TEAMY_DISCORD_OUTPUT_DIR",
    ] {
        command.env_remove(name);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Win32 CREATE_NO_WINDOW: fixture subprocesses never open a visible console.
        command.creation_flags(0x0800_0000)
    };
    let mut child = command
        .env("TEAMY_DISCORD_HOME_DIR", home)
        .env("TEAMY_DISCORD_CACHE_DIR", home.join("cache"))
        .args(["--log-filter", filter])
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn CLI fixture");
    let stdout = child.stdout.take().expect("Missing fixture stdout pipe");
    let stderr = child.stderr.take().expect("Missing fixture stderr pipe");
    let (sender, receiver) = std::sync::mpsc::channel();
    let stderr_sender = sender.clone();
    std::thread::spawn(move || {
        let mut reader = stdout;
        let mut bytes = Vec::new();
        let result = reader.read_to_end(&mut bytes).map(|_| bytes);
        let _ = sender.send((true, result));
    });
    std::thread::spawn(move || {
        let mut reader = stderr;
        let mut bytes = Vec::new();
        let result = reader.read_to_end(&mut bytes).map(|_| bytes);
        let _ = stderr_sender.send((false, result));
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().expect("Failed to inspect CLI fixture") {
            break status;
        }
        if Instant::now() >= deadline {
            child
                .kill()
                .expect("Failed to terminate timed-out CLI fixture");
            child.wait().expect("Failed to reap timed-out CLI fixture");
            panic!("CLI fixture exceeded its 10-second deadline");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let mut stdout = None;
    let mut stderr = None;
    while stdout.is_none() || stderr.is_none() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let (is_stdout, bytes) = receiver
            .recv_timeout(remaining)
            .expect("CLI fixture output did not drain before its deadline");
        let bytes = bytes.expect("Failed to drain CLI fixture output");
        if is_stdout {
            stdout = Some(bytes);
        } else {
            stderr = Some(bytes);
        }
    }
    Output {
        status,
        stdout: stdout.expect("Missing drained fixture stdout"),
        stderr: stderr.expect("Missing drained fixture stderr"),
    }
}

fn success(output: Output) -> Output {
    assert!(
        output.status.success(),
        "status {:?}; diagnostics {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn root_and_all_nested_watch_help_are_inert_and_expose_typed_arguments() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home-must-not-be-created");
    let log = temp
        .path()
        .join("logs-must-not-be-created")
        .join("help.log");
    let log = log.to_str().unwrap();
    let cases: &[(&[&str], &[&str])] = &[
        (
            &["watch"],
            &["daemon", "next", "consumer", "claim", "outbox"],
        ),
        (&["watch", "daemon"], &["serve", "status", "stop"]),
        (
            &["watch", "daemon", "serve"],
            &[
                "state-dir",
                "author-id",
                "bot-id",
                "max-pages",
                "backfill-after-id",
                "offline",
                "profile",
                "plan",
            ],
        ),
        (
            &["watch", "daemon", "status"],
            &["state-dir", "request-id", "plan"],
        ),
        (
            &["watch", "daemon", "stop"],
            &["state-dir", "request-id", "plan"],
        ),
        (
            &["watch", "next"],
            &["state-dir", "consumer", "request-id", "plan"],
        ),
        (&["watch", "consumer"], &["create"]),
        (
            &["watch", "consumer", "create"],
            &["state-dir", "consumer", "from", "now", "oldest", "plan"],
        ),
        (&["watch", "claim"], &["inspect", "replay"]),
        (
            &["watch", "claim", "inspect"],
            &["state-dir", "consumer", "plan"],
        ),
        (
            &["watch", "claim", "replay"],
            &["state-dir", "consumer", "delivery-id", "plan"],
        ),
        (&["watch", "outbox"], &["ack"]),
        (
            &["watch", "outbox", "ack"],
            &[
                "state-dir",
                "consumer",
                "event-id",
                "delivery-id",
                "receipt",
                "plan",
            ],
        ),
    ];
    for (path, expected) in cases {
        let mut args = vec!["--log-file", log];
        args.extend_from_slice(path);
        args.push("--help");
        let output = success(run(&home, &args));
        let help = String::from_utf8(output.stdout).unwrap();
        for flag in *expected {
            assert!(help.contains(flag), "help for {path:?} omitted {flag}");
        }
        if path.len() > 1 && path[1] != "daemon" {
            assert!(!help.contains("--bot-token"));
            assert!(!help.contains("--credential-provider"));
        }
        assert!(!home.exists());
        assert!(!Path::new(log).exists());
    }
    success(run(&home, &["--log-file", log, "--version"]));
    assert!(!home.exists());
    assert!(!Path::new(log).exists());
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "Keep the local IPC plan variants and their shared assertions in one end-to-end fixture."
)]
fn every_local_client_plan_is_one_typed_json_value_without_state_or_ipc_effects() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home-must-not-be-created");
    let state = temp.path().join("state-must-not-be-created");
    let state = state.to_str().unwrap();
    let cases: &[(&[&str], &str)] = &[
        (&["watch", "daemon", "status"], "status"),
        (&["watch", "daemon", "stop"], "stop"),
        (&["watch", "next", "--consumer", "reviewer"], "next"),
        (
            &[
                "watch",
                "consumer",
                "create",
                "--consumer",
                "reviewer",
                "--from",
                "now",
            ],
            "create-now",
        ),
        (
            &[
                "watch",
                "consumer",
                "create",
                "--consumer",
                "reviewer",
                "--from",
                "oldest",
            ],
            "create-oldest",
        ),
        (
            &["watch", "claim", "inspect", "--consumer", "reviewer"],
            "inspect",
        ),
        (
            &[
                "watch",
                "claim",
                "replay",
                "--consumer",
                "reviewer",
                "--delivery-id",
                DELIVERY_ID,
            ],
            "replay",
        ),
        (
            &[
                "watch",
                "outbox",
                "ack",
                "--consumer",
                "reviewer",
                "--event-id",
                "31",
                "--delivery-id",
                DELIVERY_ID,
                "--receipt",
                "fixture-handled",
            ],
            "ack",
        ),
    ];
    for (path, kind) in cases {
        let mut args = path.to_vec();
        args.extend(["--state-dir", state, "--request-id", REQUEST_ID, "--plan"]);
        let output = success(run(&home, &args));
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert_eq!(
            stdout.lines().count(),
            1,
            "{kind} emitted more than one JSON line"
        );
        let plan: WatchClientPlan = facet_json::from_str(stdout.trim()).unwrap();
        assert_eq!(plan.state_dir, state);
        assert_eq!(plan.request.version, PROTOCOL_VERSION);
        assert_eq!(plan.request.request_id.as_str(), REQUEST_ID);
        match (*kind, plan.request.command) {
            ("status", DaemonCommand::Status) | ("stop", DaemonCommand::Stop) => {}
            ("next", DaemonCommand::Next(consumer))
            | ("inspect", DaemonCommand::Inspect(consumer)) => {
                assert_eq!(consumer.as_str(), "reviewer");
            }
            (
                "create-now",
                DaemonCommand::CreateConsumer {
                    consumer,
                    start: ConsumerStart::Now,
                },
            )
            | (
                "create-oldest",
                DaemonCommand::CreateConsumer {
                    consumer,
                    start: ConsumerStart::Oldest,
                },
            ) => assert_eq!(consumer.as_str(), "reviewer"),
            (
                "replay",
                DaemonCommand::Replay {
                    consumer,
                    delivery_id,
                },
            ) => {
                assert_eq!(consumer.as_str(), "reviewer");
                assert_eq!(delivery_id.as_str(), DELIVERY_ID);
            }
            (
                "ack",
                DaemonCommand::Ack {
                    consumer,
                    event_id,
                    delivery_id,
                    receipt,
                },
            ) => {
                assert_eq!(consumer.as_str(), "reviewer");
                assert_eq!(event_id.get(), 31);
                assert_eq!(delivery_id.as_str(), DELIVERY_ID);
                assert_eq!(receipt, "fixture-handled");
            }
            _ => panic!("plan selected the wrong typed operation for {kind}"),
        }
        assert!(!home.exists());
        assert!(!Path::new(state).exists());
    }
}

#[test]
fn serve_plan_resolves_profile_scope_and_bounds_without_credentials_or_state() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    std::fs::create_dir(&home).unwrap();
    std::fs::write(home.join("profiles.json"), r#"{"version":1,"active":"fixture","profiles":[{"name":"fixture","guild-id":10,"channel-id":20}]}"#).unwrap();
    let state = temp.path().join("state");
    let state = state.to_str().unwrap();
    for offline in [false, true] {
        let mut args = vec![
            "watch",
            "daemon",
            "serve",
            "--state-dir",
            state,
            "--author-id",
            "7",
            "--bot-id",
            "42",
            "--max-pages",
            "3",
            "--backfill-after-id",
            "100",
            "--plan",
        ];
        if offline {
            args.push("--offline");
        }
        let output = success(run(&home, &args));
        let stdout = String::from_utf8(output.stdout).unwrap();
        let plan: teamy_discord::cli::watch::daemon::serve::WatchDaemonServePlan =
            facet_json::from_str(stdout.trim()).unwrap();
        assert_eq!(plan.scope.guild_id.get(), 10);
        assert_eq!(plan.scope.channel_id.get(), 20);
        assert_eq!(plan.scope.author_id.get(), 7);
        assert_eq!(plan.scope.bot_id.get(), 42);
        assert_eq!(plan.max_pages, 3);
        assert_eq!(plan.backfill_after_id.unwrap().get(), 100);
        assert_eq!(plan.offline, offline);
        assert!(!plan.query.discord_permissions_verified);
        assert!(!Path::new(state).exists());
        assert_eq!(std::fs::read_dir(&home).unwrap().count(), 1);
    }
}

#[test]
fn invalid_request_values_fail_before_connecting_or_creating_state() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home-must-not-be-created");
    let state = temp.path().join("state-must-not-be-created");
    let state = state.to_str().unwrap();
    for extra in [
        vec!["--consumer", "INVALID PRIVATE_SENTINEL"],
        vec!["--consumer", "reviewer", "--request-id", "PRIVATE_SENTINEL"],
    ] {
        let mut args = vec!["watch", "next", "--state-dir", state, "--plan"];
        args.extend(extra);
        let output = run(&home, &args);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("PRIVATE_SENTINEL"));
        assert!(!home.exists());
        assert!(!Path::new(state).exists());
    }
    for bound in ["0", "1001"] {
        let output = run(
            &home,
            &[
                "watch",
                "daemon",
                "serve",
                "--state-dir",
                state,
                "--guild-id",
                "10",
                "--channel-id",
                "20",
                "--author-id",
                "7",
                "--bot-id",
                "42",
                "--max-pages",
                bound,
                "--offline",
                "--plan",
            ],
        );
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(!Path::new(state).exists());
    }
}

#[test]
fn failed_daemon_outcomes_emit_correlated_safe_diagnostics_in_both_log_formats() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home-must-not-be-created");
    let state = temp.path().join("state-must-not-be-created");
    let state = state.to_str().unwrap();
    for format in ["text", "ndjson"] {
        let output = run_with_filter(
            &home,
            &[
                "--log-format",
                format,
                "watch",
                "outbox",
                "ack",
                "--state-dir",
                state,
                "--consumer",
                "reviewer",
                "--event-id",
                "31",
                "--delivery-id",
                DELIVERY_ID,
                "--receipt",
                "PRIVATE_SENTINEL_HANDLING_RECEIPT",
                "--request-id",
                REQUEST_ID,
            ],
            "info",
        );
        assert_eq!(output.status.code(), Some(1));
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert_eq!(stdout.lines().count(), 1);
        let response: teamy_discord::watch_daemon::DaemonResponse =
            facet_json::from_str(stdout.trim()).unwrap();
        assert_eq!(response.request_id.as_str(), REQUEST_ID);
        assert_eq!(response.exit_code(), 1);
        let diagnostics = String::from_utf8(output.stderr).unwrap();
        assert!(diagnostics.contains(REQUEST_ID));
        assert!(diagnostics.contains("request_id"));
        assert!(diagnostics.contains("outcome"));
        assert!(diagnostics.contains("daemon"));
        assert!(diagnostics.contains("Watcher daemon request failed"));
        assert!(!diagnostics.contains("PRIVATE_SENTINEL_HANDLING_RECEIPT"));
        assert!(!stdout.contains("PRIVATE_SENTINEL_HANDLING_RECEIPT"));
        if format == "ndjson" {
            assert!(
                diagnostics
                    .lines()
                    .all(|line| line.starts_with('{') && line.ends_with('}'))
            );
            assert!(!diagnostics.contains('\u{1b}'));
        }
        assert!(!home.exists());
        assert!(!Path::new(state).exists());
    }
}

#[derive(Default)]
struct CheckedWriter {
    bytes: Vec<u8>,
    fail_write: bool,
    fail_flush: bool,
    flushed: bool,
}

impl Write for CheckedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.fail_write {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "fixture write failure",
            ));
        }
        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.fail_flush {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "fixture flush failure",
            ));
        }
        self.flushed = true;
        Ok(())
    }
}

#[test]
fn json_result_writer_checks_writes_and_flushes_before_success() {
    let request = DaemonRequest {
        version: PROTOCOL_VERSION,
        request_id: ConsumerRequestId::new(REQUEST_ID).unwrap(),
        command: DaemonCommand::Next(ConsumerName::new("reviewer").unwrap()),
    };
    let mut good = CheckedWriter::default();
    render_response(&mut good, &request).unwrap();
    assert!(good.flushed);
    let stdout = String::from_utf8(good.bytes).unwrap();
    assert_eq!(stdout.lines().count(), 1);
    let parsed: DaemonRequest = facet_json::from_str(stdout.trim()).unwrap();
    assert_eq!(parsed.request_id.as_str(), REQUEST_ID);
    for mut bad in [
        CheckedWriter {
            fail_write: true,
            ..CheckedWriter::default()
        },
        CheckedWriter {
            fail_flush: true,
            ..CheckedWriter::default()
        },
    ] {
        let error = render_response(&mut bad, &request).unwrap_err();
        assert!(
            error
                .chain()
                .any(|source| source.downcast_ref::<io::Error>().is_some())
        );
        assert!(!bad.flushed);
    }
}
