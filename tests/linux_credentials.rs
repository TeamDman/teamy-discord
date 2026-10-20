#![cfg(not(windows))]
use teamy_discord::discord::credential_options::CredentialOptions;
use teamy_discord::discord::credential_selection::CredentialSelection;
use teamy_discord::discord::credential_selection::select;

#[tokio::test]
async fn direct_fixture_is_memory_only_and_redacted() {
    let options = CredentialOptions::default();
    let http = options
        .connect(Some("SYNTHETIC_NOT_A_TOKEN"), "fixture", None, None)
        .await
        .unwrap();
    assert!(!format!("{http:?}").contains("SYNTHETIC"));
    assert_eq!(http.run(async { Ok(42) }).await.unwrap(), 42);
}

#[tokio::test]
async fn all_broker_providers_fail_closed_without_starting_processes() {
    for provider in [
        "teamy-broker",
        "windows-credential-manager",
        "well-known-file",
    ] {
        let options = CredentialOptions {
            credential_provider: Some(provider.into()),
            broker_executable: Some("/must/not/run".into()),
            ..Default::default()
        };
        let error = options
            .connect(None, "fixture", None, None)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("Windows broker transport"));
        assert!(!format!("{error:?}").contains("/must/not/run"));
    }
}

#[test]
fn conflicts_and_invalid_sources_never_fall_back() {
    for values in [
        (Some("SYNTHETIC"), Some("SYNTHETIC"), None, None, None),
        (Some(""), None, None, None, None),
        (Some("SYNTHETIC"), None, None, Some("teamy-broker"), None),
        (None, None, None, Some("teamy-broker"), Some("teamy-broker")),
        (None, None, None, Some("invalid"), None),
    ] {
        assert!(select(values.0, values.1, values.2, values.3, values.4).is_err());
    }
    assert!(matches!(
        select(None, None, None, None, None).unwrap(),
        CredentialSelection::Provider("teamy-broker")
    ));
}

#[test]
fn cli_reports_unsupported_transport_as_safe_ndjson() {
    let home = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_teamy-discord"))
        .env("TEAMY_DISCORD_HOME_DIR", home.path())
        .env_remove("TEAMY_DISCORD_BOT_TOKEN")
        .env_remove("TEAMY_DISCORD_ARCHIVE_DISCORD_BOT_TOKEN")
        .env_remove("TEAMY_DISCORD_CREDENTIAL_PROVIDER")
        .args([
            "--log-filter",
            "off",
            "--log-format",
            "ndjson",
            "message",
            "list",
            "--guild-id",
            "10",
            "--channel-id",
            "20",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let record: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(record["event_kind"], "failure");
    assert!(
        record["message"]
            .as_str()
            .unwrap()
            .contains("Windows broker transport")
    );
    assert!(!home.path().join("profiles.json").exists());
}
