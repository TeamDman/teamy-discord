#![cfg(windows)]
use std::time::Duration;
use teamy_broker_sdk::Backend;
use teamy_broker_sdk::Client;
use teamy_broker_sdk::protocol::CAPABILITIES;
use teamy_broker_sdk::protocol::Hello;
use teamy_broker_sdk::protocol::Request;
use teamy_broker_sdk::protocol::Response;
use teamy_broker_sdk::protocol::VERSION;
use teamy_broker_sdk::protocol::wire;
use teamy_discord::discord::broker_handoff::acquire;
use teamy_discord::discord::credential_selection::CredentialSelection;
use teamy_discord::discord::credential_selection::select;
use tokio::net::windows::named_pipe::ServerOptions;
#[test]
// cli[verify auth.live-token.ambiguous-inputs-rejected]
// cli[verify auth.live-token.broker-memory-default]
fn conflicts_are_errors_even_when_equal_blank_or_invalid() {
    for values in [
        (Some("SYNTHETIC"), Some("SYNTHETIC"), None, None, None),
        (None, Some("SYNTHETIC"), Some("SYNTHETIC"), None, None),
        (Some(""), None, None, None, None),
        (Some("SYNTHETIC"), None, None, Some("teamy-broker"), None),
        (None, None, None, Some("teamy-broker"), Some("teamy-broker")),
        (None, None, None, Some("invalid"), None),
    ] {
        let _ = select(values.0, values.1, values.2, values.3, values.4).unwrap_err();
    }
    assert!(matches!(
        select(None, None, None, None, None).unwrap(),
        CredentialSelection::Provider("teamy-broker")
    ));
    for provider in [
        "teamy-broker",
        "windows-credential-manager",
        "well-known-file",
    ] {
        assert!(matches!(
            select(None, None, None, Some(provider), None).unwrap(),
            CredentialSelection::Provider(_)
        ));
    }
}
fn fixture(granted: bool, remaining: u64) -> (Client, tokio::task::JoinHandle<Request>) {
    fixture_failure(
        granted,
        remaining,
        teamy_broker_sdk::FailureKind::UserDenied,
    )
}
fn fixture_failure(
    granted: bool,
    remaining: u64,
    kind: teamy_broker_sdk::FailureKind,
) -> (Client, tokio::task::JoinHandle<Request>) {
    let endpoint = format!(r"\\.\pipe\teamy-discord-synthetic-{}", uuid::Uuid::new_v4());
    let mut pipe = ServerOptions::new()
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .create(&endpoint)
        .unwrap();
    let task = tokio::spawn(async move {
        pipe.connect().await.unwrap();
        wire::write(
            &mut pipe,
            &Hello {
                version: VERSION,
                capabilities: CAPABILITIES
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect(),
                instance_id: uuid::Uuid::new_v4().to_string(),
            },
        )
        .await
        .unwrap();
        let _: Hello = wire::read(&mut pipe).await.unwrap();
        let request: Request = wire::read(&mut pipe).await.unwrap();
        let reply = if granted {
            Response {
                code: "granted".into(),
                lease_id: Some(uuid::Uuid::new_v4().to_string()),
                expires_unix_ms: Some(wire::unix_ms().unwrap() + remaining),
                remaining_millis: Some(remaining),
                status: None,
                failure: None,
            }
        } else {
            Response::failure(kind)
        };
        wire::write(&mut pipe, &reply).await.unwrap();
        if granted {
            wire::write_secret(&mut pipe, b"SYNTHETIC_NOT_A_REAL_TOKEN")
                .await
                .unwrap();
        }
        request
    });
    (
        Client::new(None)
            .unwrap()
            .with_endpoint(endpoint)
            .with_deadlines(Duration::from_secs(1), Duration::from_secs(2)),
        task,
    )
}
#[tokio::test]
async fn memory_handoff_preserves_exact_context_and_redacts_debug() {
    let (client, task) = fixture(true, 1000);
    let http = acquire(
        &client,
        "synthetic-account".into(),
        "op://Synthetic/Bot/credential".into(),
        "message.list",
        Some(10),
        Some(20),
    )
    .await
    .unwrap();
    let request = task.await.unwrap().acquire.unwrap();
    assert_eq!(request.backend, Backend::Memory);
    assert_eq!(request.requesting_app, "teamy-discord");
    assert_eq!(
        request.destination,
        "https://discord.com/api/v10/channels/20"
    );
    assert_eq!(request.secret_ref, "op://Synthetic/Bot/credential");
    assert!(request.purpose.contains("message.list"));
    assert!(!format!("{http:?}").contains("SYNTHETIC"));
    assert_eq!(http.run(async { Ok(3) }).await.unwrap(), 3);
}
#[tokio::test]
async fn broker_denial_has_no_direct_or_preference_fallback() {
    let (client, task) = fixture(false, 1000);
    let error = acquire(
        &client,
        "synthetic-account".into(),
        "op://Synthetic/Bot/credential".into(),
        "message.list",
        Some(10),
        Some(20),
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("denied"));
    task.await.unwrap();
}
#[tokio::test]
async fn fixed_expiry_cancels_pending_consumer_work_without_renewal() {
    let (client, task) = fixture(true, 800);
    let http = acquire(
        &client,
        "synthetic-account".into(),
        "op://Synthetic/Bot/credential".into(),
        "message.list",
        Some(10),
        Some(20),
    )
    .await
    .unwrap();
    task.await.unwrap();
    let start = std::time::Instant::now();
    assert!(
        http.run(async {
            tokio::time::sleep(Duration::from_secs(2)).await;
            Ok(())
        })
        .await
        .is_err()
    );
    assert!(start.elapsed() < Duration::from_millis(1500));
    let _ = http.check().unwrap_err();
}

#[test]
fn live_cli_never_reads_a_saved_preference_and_plan_is_credential_free() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("discord-bot-token.txt"),
        "SYNTHETIC_SAVED_TOKEN_MUST_NOT_BE_USED",
    )
    .unwrap();
    let make = || {
        let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_teamy-discord"));
        for key in [
            "TEAMY_DISCORD_BOT_TOKEN",
            "TEAMY_DISCORD_ARCHIVE_DISCORD_BOT_TOKEN",
            "TEAMY_DISCORD_CREDENTIAL_PROVIDER",
            "TEAMY_DISCORD_BROKER_ACCOUNT",
            "TEAMY_DISCORD_BROKER_SECRET_REF",
            "TEAMY_DISCORD_BROKER_EXECUTABLE",
            "TEAMY_DISCORD_CREDENTIAL_TARGET",
            "TEAMY_DISCORD_CREDENTIAL_LEASE_ID",
        ] {
            cmd.env_remove(key);
        }
        cmd.env("TEAMY_DISCORD_HOME_DIR", dir.path());
        cmd
    };
    let denied = make()
        .args([
            "message",
            "list",
            "--guild-id",
            "10",
            "--channel-id",
            "20",
            "--limit",
            "3",
        ])
        .output()
        .unwrap();
    assert!(!denied.status.success());
    assert!(denied.stdout.is_empty());
    let diagnostics = String::from_utf8(denied.stderr).unwrap();
    assert!(diagnostics.contains("BROKER_ACCOUNT"));
    assert!(diagnostics.contains("Legacy plaintext token storage detected"));
    assert!(!diagnostics.contains("SYNTHETIC_SAVED_TOKEN"));
    let planned = make()
        .args([
            "message",
            "--plan",
            "list",
            "--guild-id",
            "10",
            "--channel-id",
            "20",
            "--limit",
            "3",
        ])
        .output()
        .unwrap();
    assert!(planned.status.success());
    assert!(
        String::from_utf8_lossy(&planned.stderr)
            .contains("Legacy plaintext token storage detected")
    );
    assert!(
        !String::from_utf8(planned.stdout)
            .unwrap()
            .contains("SYNTHETIC_SAVED_TOKEN")
    );
}

#[tokio::test]
async fn consumer_preserves_each_safe_typed_broker_failure() {
    for kind in [
        teamy_broker_sdk::FailureKind::UserDenied,
        teamy_broker_sdk::FailureKind::ApprovalTimedOut,
        teamy_broker_sdk::FailureKind::UpstreamReadFailed,
    ] {
        let (client, task) = fixture_failure(false, 1000, kind);
        let error = acquire(
            &client,
            "synthetic-account".into(),
            "op://Synthetic/Bot/credential".into(),
            "message.list",
            Some(10),
            Some(20),
        )
        .await
        .unwrap_err();
        assert_eq!(
            error.downcast_ref::<teamy_broker_sdk::FailureKind>(),
            Some(&kind)
        );
        assert!(!format!("{error:?}").contains("op://"));
        task.await.unwrap();
    }
}
