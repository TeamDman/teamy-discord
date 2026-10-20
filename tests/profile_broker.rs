use std::process::Command;
use std::process::Output;
const ACCOUNT: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAA";
const REFERENCE: &str = "op://Synthetic/Bot/credential";
fn run(home: &std::path::Path, args: &[&str]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_teamy-discord"));
    for name in [
        "TEAMY_DISCORD_BOT_TOKEN",
        "TEAMY_DISCORD_ARCHIVE_DISCORD_BOT_TOKEN",
        "TEAMY_DISCORD_BROKER_ACCOUNT",
        "TEAMY_DISCORD_BROKER_SECRET_REF",
        "TEAMY_DISCORD_CREDENTIAL_PROVIDER",
        "TEAMY_DISCORD_OUTPUT_DIR",
    ] {
        cmd.env_remove(name);
    }
    cmd.env("TEAMY_DISCORD_HOME_DIR", home)
        .args(["--log-filter", "off"])
        .args(args)
        .output()
        .unwrap()
}
fn success(home: &std::path::Path, args: &[&str]) -> Output {
    let out = run(home, args);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}
#[test]
fn old_profiles_survive_metadata_changes_and_short_plan_resolves_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    std::fs::write(home.join("profiles.json"),r#"{"version":1,"active":"other","profiles":[{"name":"sfm","guild-id":10,"channel-id":20},{"name":"other","guild-id":30,"channel-id":40}]}"#).unwrap();
    success(
        home,
        &[
            "profile",
            "broker",
            "set",
            "sfm",
            "--account",
            ACCOUNT,
            "--secret-ref",
            REFERENCE,
        ],
    );
    success(
        home,
        &["profile", "broker", "set", "sfm", "--account", ACCOUNT],
    );
    success(
        home,
        &[
            "profile",
            "set",
            "sfm",
            "--guild-id",
            "10",
            "--channel-id",
            "20",
        ],
    );
    let saved =
        teamy_discord::paths::Profiles::load(&teamy_discord::paths::AppHome(home.to_owned()))
            .unwrap();
    assert_eq!(saved.active.as_deref(), Some("other"));
    assert_eq!(saved.profiles.len(), 2);
    assert_eq!(saved.named("other").unwrap().guild_id, 30);
    assert_eq!(
        saved
            .named("sfm")
            .unwrap()
            .broker
            .as_ref()
            .unwrap()
            .secret_ref
            .as_deref(),
        Some(REFERENCE)
    );
    success(home, &["profile", "use", "sfm"]);
    let plan = success(home, &["message", "--plan", "list", "--limit", "10"]);
    let plan: serde_json::Value = serde_json::from_slice(&plan.stdout).unwrap();
    assert_eq!(plan["guild-id"], 10);
    assert_eq!(plan["channel-id"], 20);
    assert_eq!(plan["profile-broker-configured"], true);
    assert!(!String::from_utf8_lossy(&plan.to_string().into_bytes()).contains(REFERENCE));
    success(home, &["profile", "broker", "clear", "sfm", "--secret-ref"]);
    let cleared = success(home, &["profile", "broker", "show", "sfm"]);
    let fields: serde_json::Value = serde_json::from_slice(&cleared.stdout).unwrap();
    assert_eq!(fields["account"], ACCOUNT);
    assert!(fields["secret-ref"].is_null());
    success(home, &["profile", "broker", "clear", "sfm", "--all"]);
    let state =
        teamy_discord::paths::Profiles::load(&teamy_discord::paths::AppHome(home.to_owned()))
            .unwrap();
    assert!(state.named("sfm").unwrap().broker.is_none());
    assert!(!home.join("discord-bot-token.txt").exists());
}
#[test]
fn invalid_metadata_and_ambiguous_clear_leave_saved_document_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    success(
        home,
        &[
            "profile",
            "set",
            "sfm",
            "--guild-id",
            "10",
            "--channel-id",
            "20",
        ],
    );
    let original = std::fs::read(home.join("profiles.json")).unwrap();
    for args in [
        &["profile", "broker", "set", "sfm", "--account", "help"][..],
        &[
            "profile",
            "broker",
            "set",
            "sfm",
            "--secret-ref",
            "SECRET_SENTINEL",
        ][..],
        &["profile", "broker", "clear", "sfm", "--all", "--account"][..],
    ] {
        let out = run(home, args);
        assert!(!out.status.success());
        assert!(!String::from_utf8_lossy(&out.stderr).contains("SECRET_SENTINEL"));
        assert_eq!(std::fs::read(home.join("profiles.json")).unwrap(), original);
    }
}
#[test]
fn profile_broker_help_uses_the_accepted_hyphenated_reference_flag() {
    let dir = tempfile::tempdir().unwrap();
    for command in ["set", "clear"] {
        let output = success(dir.path(), &["profile", "broker", command, "--help"]);
        let help = String::from_utf8(output.stdout).unwrap();
        assert!(help.contains("--secret-ref"));
        assert!(!help.contains("--secret_ref"));
    }
    assert!(!dir.path().join("profiles.json").exists());
}
#[test]
fn parser_help_and_missing_context_errors_are_specific_and_do_not_read_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    for args in [
        &["message", "list", "--broker-account", "--help"][..],
        &["message", "list", "--broker-secret-ref", "--help"][..],
    ] {
        let out = success(home, args);
        assert!(String::from_utf8_lossy(&out.stdout).contains("USAGE"));
    }
    let missing = run(home, &["message", "list", "--broker-account"]);
    assert!(String::from_utf8_lossy(&missing.stderr).contains("--broker-account requires a value"));
    let guild = run(home, &["message", "list", "--limit", "10"]);
    assert!(String::from_utf8_lossy(&guild.stderr).contains("Provide --guild-id"));
    let channel = run(home, &["message", "list", "--guild-id", "10"]);
    assert!(String::from_utf8_lossy(&channel.stderr).contains("Provide --channel-id"));
    let invalid = run(home, &["message", "list", "--guild-id", "PRIVATE_SENTINEL"]);
    let text = String::from_utf8_lossy(&invalid.stderr);
    assert!(text.contains("Invalid value for --guild-id"));
    assert!(!text.contains("PRIVATE_SENTINEL"));
    let unknown = run(home, &["--PRIVATE_SENTINEL=value"]);
    assert!(!String::from_utf8_lossy(&unknown.stderr).contains("PRIVATE_SENTINEL"));
    assert!(!home.join("profiles.json").exists());
}
#[test]
fn removed_storage_commands_have_no_alias_or_secret_side_effects() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    for command in ["set", "clear", "show-source", "validate"] {
        let denied = run(home, &["bot-token", command, "PRIVATE_SENTINEL"]);
        assert!(!denied.status.success());
        let diagnostic = String::from_utf8_lossy(&denied.stderr);
        assert!(diagnostic.contains("bot-token command was removed"));
        assert!(diagnostic.contains("profile broker"));
        assert!(!diagnostic.contains("PRIVATE_SENTINEL"));
        assert!(!home.join("discord-bot-token.txt").exists());
    }
    let help = success(home, &["--help"]);
    assert!(!String::from_utf8_lossy(&help.stdout).contains("bot-token"));
}
#[test]
fn existing_plaintext_file_produces_safe_migration_warning_without_being_read_or_changed() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let path = home.join("discord-bot-token.txt");
    std::fs::write(&path, "PRIVATE_SENTINEL").unwrap();
    let result = success(home, &["home", "show"]);
    let diagnostic = String::from_utf8_lossy(&result.stderr);
    assert!(diagnostic.contains("Legacy plaintext token storage detected"));
    assert!(!diagnostic.contains("PRIVATE_SENTINEL"));
    assert_eq!(std::fs::read(&path).unwrap(), b"PRIVATE_SENTINEL");
    let json = success(home, &["--log-format", "ndjson", "home", "show"]);
    let warning: serde_json::Value = serde_json::from_slice(&json.stderr).unwrap();
    assert_eq!(warning["event_kind"], "migration");
    assert_eq!(warning["level"], "WARN");
    // A directory cannot be read as a token; only metadata detection is allowed.
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    let result = success(home, &["home", "show"]);
    assert!(String::from_utf8_lossy(&result.stderr).contains("Legacy plaintext"));
    assert!(path.is_dir());
}
#[test]
fn one_off_bot_token_surface_has_no_token_alias_and_invalid_input_never_uses_profile_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    success(
        home,
        &[
            "profile",
            "set",
            "sfm",
            "--guild-id",
            "10",
            "--channel-id",
            "20",
        ],
    );
    success(
        home,
        &[
            "profile",
            "broker",
            "set",
            "sfm",
            "--account",
            ACCOUNT,
            "--secret-ref",
            REFERENCE,
        ],
    );
    success(home, &["profile", "use", "sfm"]);
    let help = success(home, &["message", "--help"]);
    let text = String::from_utf8_lossy(&help.stdout);
    assert!(text.contains("--bot-token"));
    assert!(!text.contains("--token"));
    let plan = success(
        home,
        &[
            "message",
            "--bot-token",
            "SYNTHETIC_TOKEN",
            "--plan",
            "list",
        ],
    );
    assert!(!String::from_utf8_lossy(&plan.stdout).contains("SYNTHETIC_TOKEN"));
    for args in [
        vec!["message", "--bot-token", "", "list"],
        vec!["message", "--bot-token", "PRIVATE\nSENTINEL", "list"],
        vec!["message", "--token", "PRIVATE_SENTINEL", "list"],
    ] {
        let denied = run(home, &args);
        assert!(!denied.status.success());
        assert!(!String::from_utf8_lossy(&denied.stderr).contains("PRIVATE_SENTINEL"));
        assert!(!String::from_utf8_lossy(&denied.stderr).contains("PRIVATE\nSENTINEL"));
    }
    let mut conflict = Command::new(env!("CARGO_BIN_EXE_teamy-discord"));
    let result = conflict
        .env("TEAMY_DISCORD_HOME_DIR", home)
        .env("TEAMY_DISCORD_BOT_TOKEN", "same")
        .env_remove("TEAMY_DISCORD_ARCHIVE_DISCORD_BOT_TOKEN")
        .args([
            "--log-filter",
            "off",
            "message",
            "--bot-token",
            "same",
            "list",
        ])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("Conflicting credential inputs"));
    assert!(!home.join("discord-bot-token.txt").exists());
}

#[test]
fn mentions_cli_help_unknown_context_and_payload_redaction_regressions() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let exact = [
        "message",
        "list",
        "--guild-id",
        "10",
        "--channel-id",
        "20",
        "--limit",
        "10",
        "--mentions",
        "me",
    ];
    let help_args: Vec<_> = exact.into_iter().chain(["--help"]).collect();
    let help = success(home, &help_args);
    assert!(String::from_utf8_lossy(&help.stdout).contains("--mentions"));
    assert!(String::from_utf8_lossy(&help.stdout).contains("--scan-limit"));
    let plan_args: Vec<_> = exact.into_iter().chain(["--plan"]).collect();
    let plan = success(home, &plan_args);
    let decoded: serde_json::Value = serde_json::from_slice(&plan.stdout).unwrap();
    assert_eq!(decoded["mentions"], "me");
    assert_eq!(decoded["limit"], 10);
    assert_eq!(decoded["scan-limit"], 100);
    assert_eq!(decoded["identity-resolution"], "requires-live-users/@me");
    let failure = run(home, &["message", "list", "--token=PRIVATE_SENTINEL"]);
    assert!(!failure.status.success());
    let error = String::from_utf8_lossy(&failure.stderr);
    assert!(error.contains("Unknown option --token for message list"));
    assert!(error.contains("message list --help"));
    assert!(!error.contains("PRIVATE_SENTINEL"));
    for args in [
        vec!["message", "list", "--PRIVATE_SENTINEL=value", "--help"],
        vec!["message", "list", "--token", "PRIVATE_SENTINEL", "--help"],
    ] {
        let help = success(home, &args);
        assert!(String::from_utf8_lossy(&help.stdout).contains("USAGE"));
        assert!(!String::from_utf8_lossy(&help.stdout).contains("PRIVATE_SENTINEL"));
    }
    assert!(!home.join("profiles.json").exists());
}
