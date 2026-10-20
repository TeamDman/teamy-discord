use facet::Facet;
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Facet)]
struct LogLine {
    level: String,
}

fn command(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_teamy-discord"));
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("TEAMY_DISCORD_") {
            command.env_remove(name);
        }
    }
    command
        .env("TEAMY_DISCORD_HOME_DIR", root.join("home"))
        .env("TEAMY_DISCORD_CACHE_DIR", root.join("cache"))
        .env_remove("RUST_LOG");
    command
}

#[test]
fn independent_defaults_keep_routine_events_in_debug_file_without_generic_progress() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("capture.ndjson");
    let output = command(root.path())
        .args(["--log-format", "ndjson", "--log-file"])
        .arg(&file)
        .args(["home", "show"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    for line in stderr.lines() {
        let record: LogLine = facet_json::from_str(line).unwrap();
        assert_ne!(record.level, "DEBUG");
    }
    assert!(!stderr.contains("Task progress"));
    assert!(!stderr.contains("Command finished"));
    let file_text = fs::read_to_string(&file).unwrap();
    assert!(
        file_text
            .lines()
            .map(|line| facet_json::from_str::<LogLine>(line).unwrap())
            .any(|record| record.level == "DEBUG")
    );
    assert!(!file_text.contains('\u{1b}'));
    assert!(
        !String::from_utf8(output.stdout)
            .unwrap()
            .contains("Task progress")
    );
}

#[test]
fn stderr_off_does_not_disable_file_and_existing_files_are_appended() {
    let root = tempfile::tempdir().unwrap();
    let ambient_home = root.path().join("ambient-home");
    fs::create_dir_all(&ambient_home).unwrap();
    fs::write(
        ambient_home.join("discord-bot-token.txt"),
        "SYNTHETIC_SECRET_NEVER_LOG",
    )
    .unwrap();
    let file = root.path().join("capture.log");
    fs::write(&file, "prior record\n").unwrap();
    let output = command(root.path())
        .env("TEAMY_DISCORD_ARCHIVE_HOME_DIR", &ambient_home)
        .args([
            "--log-filter",
            "off",
            "--log-file-format",
            "text",
            "--log-file",
        ])
        .arg(&file)
        .args(["home", "show"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = fs::read_to_string(&file).unwrap();
    assert!(text.starts_with("prior record\n"));
    assert!(text.contains("Tracing initialized"));
    assert!(!text.contains('\u{1b}'));
}

#[test]
fn invalid_filters_have_no_file_effects_and_emergency_json_omits_raw_payloads() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("SYNTHETIC_SECRET_NEVER_LOG.ndjson");
    let output = command(root.path())
        .args([
            "--log-format",
            "ndjson",
            "--log-file-filter",
            "fixture=bogus",
            "--log-file",
        ])
        .arg(&file)
        .args(["home", "show"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!file.exists());
    let stderr = String::from_utf8(output.stderr).unwrap();
    for line in stderr.lines() {
        let _: LogLine = facet_json::from_str(line).unwrap();
    }
    assert!(!stderr.contains("SYNTHETIC_SECRET"));
}

#[test]
fn ndjson_parse_failures_are_safe_and_help_does_not_open_a_file() {
    let root = tempfile::tempdir().unwrap();
    let output = command(root.path())
        .args(["--log-format", "ndjson", "--unknown-SYNTHETIC_SECRET"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(!stderr.contains("SYNTHETIC_SECRET"));
    for line in stderr.lines() {
        let _: LogLine = facet_json::from_str(line).unwrap();
    }
    let file = root.path().join("must-not-exist.ndjson");
    let output = command(root.path())
        .args(["--log-format", "ndjson", "--log-file"])
        .arg(&file)
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!file.exists());
}

#[test]
fn text_parse_failure_cannot_echo_sensitive_arguments() {
    let root = tempfile::tempdir().unwrap();
    let output = command(root.path())
        .args(["--unknown-SYNTHETIC_SECRET"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        !String::from_utf8(output.stderr)
            .unwrap()
            .contains("SYNTHETIC_SECRET")
    );
}
