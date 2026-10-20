# Mention watcher

The Windows `watch` group runs one foreground daemon for read-only Discord
capture and short-lived local IPC clients. `watch next` waits for one JSON result
and exits. Capturing an event, handling it, and acknowledging it are separate
operations. The CLI does not send Discord replies or change GitHub repositories.

## Select a scope

Capture selects one guild, channel, human author, and expected bot. An event must
have that top-level author, no bot/webhook identity, and a structured user mention
of the configured bot. Live startup verifies the authenticated bot and the
guild/channel relationship. Referenced replies are checked for the required
identity and scope metadata.

Only the selected author's own message supplies an instruction candidate. Other
authors, references, quotes, and attachments remain context. A captured mention
does not authorize an external action.

All IDs and paths below are synthetic. Replace them with your intended scope.
Use an absolute state directory on a protected local filesystem. From the
repository root, inspect the current source's help with `cargo run -- watch --help`.
The examples assume that version is installed as `teamy-discord`.

```powershell
teamy-discord profile set demo --guild-id 100000000000000001 --channel-id 200000000000000001
teamy-discord watch daemon serve --profile demo --state-dir C:/discord-watch/demo --author-id 300000000000000001 --bot-id 400000000000000001 --plan
```

`--plan` validates without credentials, Discord, IPC, or state writes. Configure
the profile's nonsecret broker account/item reference as described in the
[README](../README.md#get-started). Remove `--plan` to start live capture under
normal broker approval. Keep the daemon in the foreground and run clients from
another terminal. The CLI installs no service, startup entry, or scheduled task.

Online startup acquires the broker lease, arms its expiry watchdog, verifies
scope, and starts Gateway tasks before opening capture/consumer state. Approval
failure before that point creates no watcher state. `--profile` belongs to
`daemon serve`; local clients use `--state-dir` and acquire no credentials.

First initialization saves the newest visible message as its baseline and ignores
existing history. `--backfill-after-id` supplies an exclusive lower message-ID
boundary only for a new capture state. Restart restores the saved checkpoint and
unfinished sweep; existing state rejects a new backfill boundary.

## Capture and publication

Gateway Ready, Resume, and selected-channel message-create events coalesce into
one pending notification. Their payloads wake capture; authoritative REST reads
supply the captured messages. There is no periodic HTTP history poll.

REST uses exact snowflake cursors and pages of at most 100 messages. `--max-pages`
defaults to 10 and accepts 1 through 1000 per sweep. An unfinished sweep commits
its frozen head, cursor, and old checkpoint, then continues inside the daemon.
The actor handles IPC while REST and retry operations await completion.

Each completed page commits candidates and resume progress together. Completed
coverage publishes the retained core outbox to the separate consumer store.
Source IDs deduplicate repeated publication and retain their original payloads
and local sequences. A delayed older Discord message can receive a later local
sequence. Startup repairs a crash between capture completion and publication.

Transient reads retry the interrupted capture cycle with bounded backoff; five
consecutive failures stop capture. Scope, persistence, and corrupt-state errors
fail explicitly. Visibility, edits, and deletions remain outside an audit-log
guarantee.

## Create a consumer and wait

With the daemon running:

```powershell
teamy-discord watch daemon status --state-dir C:/discord-watch/demo
teamy-discord watch consumer create --state-dir C:/discord-watch/demo --consumer review --from now
teamy-discord watch next --state-dir C:/discord-watch/demo --consumer review
```

Choose `--from now` or `--from oldest`. Creation requires a complete publication
boundary. `now` starts after the latest published sequence; `oldest` starts at
zero. Reusing a consumer name preserves its cursor. A fresh offline daemon with
no capture history allows an empty boundary at zero.

Consumers have independent acknowledged cursors. Each allows one waiting client
and one durable in-flight claim. An active waiter produces `busy`; an already
completed request produces `already-completed`; an outstanding claim produces
`pending-ack`. These results do not silently replay a payload.

Before returning an event, the actor commits the consumer, source event ID,
sequence, delivery ID, and request UUID. The client writes and flushes one JSON
result. Output, exit, disconnect, cancellation, and failed output do not
acknowledge handling. Applications invoking `next` must retain and await the
process; conversation routing or agent wake-up adapters are outside this CLI.

## Inspect, replay, and acknowledge

Inspect an uncertain claim before replaying it:

```powershell
teamy-discord watch claim inspect --state-dir C:/discord-watch/demo --consumer review
```

Only explicit `watch claim replay` returns the current claim's payload with its
original delivery ID. Claims do not automatically expire, release, or reassign;
client failure and daemon restart preserve them.

After completing external handling, acknowledge the exact IDs returned by the
delivery. For example, substitute the actual event/delivery IDs into these
variables before running the command:

```powershell
$eventId = '500000000000000001'
$deliveryId = 'd1-0000000000000000000000000000000000000000000000000000000000000000'
teamy-discord watch outbox ack --state-dir C:/discord-watch/demo --consumer review --event-id $eventId --delivery-id $deliveryId --receipt 'handled locally'
```

The nonsecret receipt must be nonempty, at most 4096 bytes, and contain no control
characters. A matching acknowledgment records completion, advances only that
consumer's cursor, and clears the claim. Identical retries succeed even after a
newer claim; conflicting receipts fail.

Optional `--request-id` is a canonical UUID for an intentional retry of the same
operation and parameters. Reusing a completed `next` request returns
`already-completed`, without a payload or new claim. Reconcile uncertain external
handling before replaying; a local receipt does not guarantee exactly-once
external effects.

## Lease expiry and offline recovery

Live capture uses the existing Windows broker providers and approval rules.
The request purpose includes scope, author, bot, and capture bounds. Reusing an
approved request preserves its original absolute 15-minute deadline.

Before Gateway construction, a dedicated thread checks the original SDK lease
every 10 ms. An invalid lease exits the whole daemon with code 3 independently
of the async runtime. Scheduling determines when the check can run. Already-sent
Discord requests cannot be recalled.

Expiry stops IPC as well as live capture. Connected clients retain a verified
server process handle and classify exit 3 as `approval-required`; a new client
after process termination reports an unavailable daemon. Explicitly restart in
offline mode to inspect retained captures and claims without credentials:

```powershell
teamy-discord watch daemon serve --offline --profile demo --state-dir C:/discord-watch/demo --author-id 300000000000000001 --bot-id 400000000000000001
```

Offline status, replay, acknowledgments, and retained deliveries use local state.
An empty offline `next` returns `offline`. Resume live capture through an explicit
regular `daemon serve` invocation and approval. There is no automatic renewal or
fallback credential source.

```powershell
teamy-discord watch daemon stop --state-dir C:/discord-watch/demo
```

## IPC, storage, and results

Windows named pipes restrict access to the current-user SID, reject remote
clients, and verify peer SIDs. The endpoint derives from the absolute state path
and SID. Same-user access is not executable attestation. Protocol version 1 uses
length-prefixed Facet JSON frames bounded to 1 MiB.

The daemon owns `daemon.lock`, `capture/`, and `consumers/`. Capture and consumer
stores use permanent OS locks and same-directory staged snapshots, syncing and
closing staging before replacement. Corrupt committed state fails explicitly;
stale staging is uncommitted. Each snapshot is limited to 64 MiB. Events, IDs,
claims, requests, and receipts are retained without pruning. Capacity/persistence
failure stops work instead of discarding records.

Protect captured Discord text in the state directory. The durability model
assumes cooperating writers on a supported local filesystem. Discord IDs are
64-bit JSON integers and require lossless parsing.

| Result | Exit code |
| --- | --- |
| matched, replayed, acknowledged, status, inspection, creation, stop | 0 |
| busy, pending-ack, already-completed, offline, error | 1 |
| approval-required | 3 |
| cancelled | 130 |

Responses contain a version, request UUID, outcome, optional delivery/claim/
consumer/acknowledgment/status, and a safe message. Cancellation can race claim
commit; inspect the claim when the outcome is uncertain.

See the [sequence diagram](mentions-watcher-sequence.mmd),
[serve arguments](../src/cli/watch/daemon/serve/watch_daemon_serve_cli.rs),
[capture source](../src/discord/watch_gateway/gateway_rest_source.rs),
[consumer store](../src/watch_consumers/consumer_store.rs), and
[IPC implementation](../src/watch_daemon/server.rs).
