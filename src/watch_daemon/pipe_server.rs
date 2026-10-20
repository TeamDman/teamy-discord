use super::DaemonHello;
use super::DaemonRequest;
use super::DaemonResponse;
use super::actor_message::ActorMessage;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
#[cfg(windows)]
use tokio::net::windows::named_pipe::NamedPipeServer;
use tokio::sync::Semaphore;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task::JoinSet;

#[cfg(windows)]
pub(super) async fn run(
    listener: NamedPipeServer,
    endpoint: String,
    hello: DaemonHello,
    commands: mpsc::Sender<ActorMessage>,
    stopped: Arc<AtomicBool>,
) -> eyre::Result<()> {
    let permits = Arc::new(Semaphore::new(32));
    let mut clients = JoinSet::new();
    // A single free instance lets frequent Status clients starve a Next client
    // that is sleeping after ERROR_PIPE_BUSY. Four bounded accept slots allow
    // concurrent clients to connect without competing for one periodic opening.
    let mut accepts = JoinSet::new();
    queue_accept(&mut accepts, listener);
    for _ in 1..4 {
        queue_accept(
            &mut accepts,
            super::pipe_security::create(&endpoint, false)?,
        );
    }
    let mut next_connection_id = 1_u64;
    let result: eyre::Result<()> = async {
        let mut check = tokio::time::interval(Duration::from_millis(25));
        loop {
            if stopped.load(Ordering::Acquire) { break; }
            tokio::select! {
                accepted = accepts.join_next() => {
                    let connected = accepted.ok_or_else(|| eyre::eyre!("Watcher accept pool stopped"))?
                        .map_err(|_join_error| eyre::eyre!("Watcher accept task failed"))??;
                    queue_accept(&mut accepts, super::pipe_security::create(&endpoint, false)?);
                    let Ok(permit) = Arc::clone(&permits).try_acquire_owned() else { drop(connected); continue; };
                    if super::pipe_security::verify_client(&connected).is_err() { continue; }
                    let connection_id = super::connection_id::ConnectionId(next_connection_id);
                    next_connection_id = next_connection_id.checked_add(1).ok_or_else(|| eyre::eyre!("Connection identity capacity reached"))?;
                    let hello = hello.clone();
                    let commands = commands.clone();
                    clients.spawn(async move {
                        let _permit = permit;
                        let _ = connection(connected, hello, commands, connection_id).await;
                    });
                }
                _ = check.tick() => {}
                Some(_) = clients.join_next(), if !clients.is_empty() => {}
            }
        }
        Ok(())
    }.await;
    accepts.abort_all();
    while accepts.join_next().await.is_some() {}
    let drained = tokio::time::timeout(Duration::from_secs(2), async {
        while clients.join_next().await.is_some() {}
    })
    .await;
    if drained.is_err() {
        clients.abort_all();
        while clients.join_next().await.is_some() {}
    }
    result
}

#[cfg(windows)]
fn queue_accept(accepts: &mut JoinSet<std::io::Result<NamedPipeServer>>, server: NamedPipeServer) {
    accepts.spawn(async move {
        server.connect().await?;
        Ok(server)
    });
}

async fn connection(
    mut stream: impl AsyncRead + AsyncWrite + Unpin,
    hello: DaemonHello,
    commands: mpsc::Sender<ActorMessage>,
    connection_id: super::connection_id::ConnectionId,
) -> eyre::Result<()> {
    tokio::time::timeout(
        Duration::from_secs(5),
        super::wire::write(&mut stream, &hello),
    )
    .await??;
    let request: DaemonRequest =
        tokio::time::timeout(Duration::from_secs(5), super::wire::read(&mut stream)).await??;
    request.validate()?;
    let (response, received) = oneshot::channel::<DaemonResponse>();
    tokio::time::timeout(
        Duration::from_secs(5),
        commands.send(ActorMessage::Request {
            connection_id,
            request,
            response,
        }),
    )
    .await??;
    let mut unexpected = [0_u8; 1];
    let result = tokio::select! {
        result = received => result,
        _ = stream.read(&mut unexpected) => {
            let _ = commands.try_send(ActorMessage::Disconnected { connection_id });
            return Ok(());
        }
    };
    let response =
        result.map_err(|_closed_actor| eyre::eyre!("Watcher actor stopped before responding"))?;
    tokio::time::timeout(
        Duration::from_secs(5),
        super::wire::write(&mut stream, &response),
    )
    .await??;
    Ok(())
}
