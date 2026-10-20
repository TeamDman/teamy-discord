use super::DaemonEndpoint;
use super::DaemonHello;
use super::DaemonOutcome;
use super::DaemonRequest;
use super::DaemonResponse;
use super::PROTOCOL_VERSION;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
#[cfg(windows)]
use tokio::net::windows::named_pipe::ClientOptions;

/// Send one typed request and await one response. No state files or credentials are read.
/// # Errors
/// Propagates structural/protocol/transport errors without logging arbitrary frame bytes.
/// One cancellable operation, including connection, handshake and request write.
pub async fn client_request(
    endpoint: &DaemonEndpoint,
    request: &DaemonRequest,
    cancelled: &AtomicBool,
) -> eyre::Result<DaemonResponse> {
    if cancelled.load(Ordering::Acquire) {
        return Ok(cancelled_response(request));
    }
    tokio::select! {
        result = client_request_work(endpoint, request, cancelled) => result,
        () = async {
            loop {
                if cancelled.load(Ordering::Acquire) { break; }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        } => Ok(cancelled_response(request)),
    }
}

async fn client_request_work(
    endpoint: &DaemonEndpoint,
    request: &DaemonRequest,
    cancelled: &AtomicBool,
) -> eyre::Result<DaemonResponse> {
    request.validate()?;
    #[cfg(not(windows))]
    {
        let _ = (endpoint, cancelled);
        eyre::bail!("Watcher IPC is supported on Windows only")
    }
    #[cfg(windows)]
    {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let mut pipe = loop {
            if cancelled.load(Ordering::Acquire) {
                return Ok(cancelled_response(request));
            }
            match ClientOptions::new().open(endpoint.as_str()) {
                Ok(pipe) => break pipe,
                Err(error)
                    if error.raw_os_error() == Some(231)
                        && tokio::time::Instant::now() < deadline =>
                {
                    tokio::time::sleep(Duration::from_millis(25)).await;
                }
                Err(_) => {
                    return Ok(DaemonResponse::new(
                        request.request_id.clone(),
                        DaemonOutcome::Error,
                        "Watcher daemon is unavailable. Start it explicitly",
                    ));
                }
            }
        };
        let peer = super::pipe_security::verify_server(&pipe)?;
        let hello: DaemonHello = match tokio::time::timeout(
            Duration::from_secs(5),
            super::wire::read(&mut pipe),
        )
        .await
        {
            Ok(Ok(hello)) => hello,
            _ => return disconnected_response(request, &peer, None).await,
        };
        eyre::ensure!(
            hello.version == PROTOCOL_VERSION,
            "Unsupported watcher IPC protocol"
        );
        hello.scope.validate()?;
        if !matches!(
            tokio::time::timeout(
                Duration::from_secs(5),
                super::wire::write(&mut pipe, request)
            )
            .await,
            Ok(Ok(()))
        ) {
            return disconnected_response(request, &peer, hello.lease_expires_unix_ms).await;
        }
        let response = super::wire::read::<DaemonResponse>(&mut pipe);
        tokio::pin!(response);
        let mut check = tokio::time::interval(Duration::from_millis(25));
        let result = loop {
            tokio::select! {
                result = &mut response => break result,
                _ = check.tick() => {
                    if cancelled.load(Ordering::Acquire) { return Ok(cancelled_response(request)); }
                }
            }
        };
        match result {
            Ok(response) => {
                eyre::ensure!(
                    response.version == PROTOCOL_VERSION
                        && response.request_id == request.request_id,
                    "Watcher IPC response identity mismatch"
                );
                if let Some(delivery) = &response.delivery {
                    eyre::ensure!(
                        delivery.message.is_trigger(&hello.scope)?
                            && delivery.claim.event_id == delivery.message.id,
                        "Watcher IPC delivery scope mismatch"
                    );
                }
                Ok(response)
            }
            Err(_) => disconnected_response(request, &peer, hello.lease_expires_unix_ms).await,
        }
    }
}
fn cancelled_response(request: &DaemonRequest) -> DaemonResponse {
    DaemonResponse::new(
        request.request_id.clone(),
        DaemonOutcome::Cancelled,
        "Client wait cancelled. Inspect consumer state if claim commit may have won the race",
    )
}
#[cfg(windows)]
fn unix_ms() -> eyre::Result<u64> {
    Ok(u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis(),
    )?)
}

#[cfg(windows)]
async fn disconnected_response(
    request: &DaemonRequest,
    peer: &super::daemon_peer_process::DaemonPeerProcess,
    wall_deadline: Option<u64>,
) -> eyre::Result<DaemonResponse> {
    let expired = peer.exited_for_approval().await
        || wall_deadline.is_some_and(|expiry| unix_ms().is_ok_and(|now| now >= expiry));
    Ok(DaemonResponse::new(
        request.request_id.clone(),
        if expired {
            DaemonOutcome::ApprovalRequired
        } else {
            DaemonOutcome::Error
        },
        if expired {
            "Daemon stopped at fixed approval expiry. No lease was renewed"
        } else {
            "Daemon connection closed or failed before a complete response. Inspect the consumer before explicit replay"
        },
    ))
}
