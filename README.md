# Teamy Discord

<!-- repo[impl readme.media-demo] -->
![Teamy Discord: Discord messages, mentions, terminal, and local archives](resources/readme-banner.png)

<!-- repo[impl readme.explanation] -->
A Rust CLI for reading Discord with a bot, saving local archives, and capturing
selected mentions for local workflows. It started as an archive tool for the
Super Factory Manager server.

- Query guilds, channels, active threads, users, messages, and attachment metadata.
- Save guild/channel metadata, messages, attachments, and message-author records
  to a resumable filesystem archive.
- Reuse named profiles and preview reads without credentials or network access.
- On Windows, capture mentions through a foreground daemon with durable local
  consumers and acknowledgments.

Discord operations read data; the CLI does not send, edit, or delete messages.
Profiles, archives, checkpoints, and watcher consumers change local state.

## Build and install

Use a current stable Rust toolchain with Cargo (Rust 2024 edition). On Windows,
install the MSVC C++ build tools and Windows SDK for compilation and executable
resources. Git is needed to clone the repositories.

The current manifest uses `../teamy-broker/sdk` as a path dependency. Keep a
compatible [teamy-broker](https://github.com/TeamDman/teamy-broker) checkout beside
this repository, including when resolving dependencies for a Linux build. The
CLI, SDK, and broker must use compatible protocols; this path dependency prevents
a standalone `cargo install --git` installation.

From a common parent directory:

```powershell
git clone https://github.com/TeamDman/teamy-broker.git
git clone https://github.com/TeamDman/teamy-discord.git
cd teamy-discord
cargo build --locked
cargo install --locked --path . --bin teamy-discord
```

`update.ps1` runs the same install command. Cargo's bin directory must be on PATH.
Broker authentication needs a compatible `teamy-broker.exe` on PATH and its
1Password setup; see the broker's README.

Examples use the current source's command layout. An installed binary can lag
behind the checkout. Check `teamy-discord --help`, or use `cargo run -- --help`
and replace `teamy-discord` with `cargo run --` in an example to run this source.
`--version` includes the build's Git revision, but does not identify uncommitted
changes. In particular, older installed builds may lack `watch`.

Linux supports profiles, local archive operations, plans, and live reads with an
explicitly injected token. Broker authentication and watcher IPC require Windows.
See [Linux setup and runtime requirements](docs/linux-support.md).

<!-- repo[impl readme.code-example] -->
## Get started

The IDs, account, and vault reference below are synthetic. Replace them with your
own bot's scope and references. Profiles store defaults and broker references,
never the returned token.

```powershell
teamy-discord profile set demo --guild-id 100000000000000001 --channel-id 200000000000000001
teamy-discord profile broker set demo --account demo@example.invalid --secret-ref 'op://Example/DiscordBot/credential'
teamy-discord profile use demo
teamy-discord message --plan list --limit 10
```

These commands save local configuration and print a query plan. `--plan` does not
resolve credentials or contact Discord. It can use historical channel/thread
metadata through `--archive-root`, but cannot verify current Discord permissions.

Once the bot has access to the intended guild and channel, perform the read:

```powershell
teamy-discord message list --limit 10
```

An explicit `--profile demo` overrides the active profile. Explicit IDs override
profile defaults; changing the guild drops an inherited channel default.
`--channel-id` and `--thread-id` are mutually exclusive. Use `profile list`,
`profile show demo`, and `profile broker show demo` to inspect defaults.
`profile clear` clears the active selection while retaining saved profiles.
Saving an existing profile replaces its guild/channel defaults; omitting
`--channel-id` clears that default while preserving broker references.
`profile broker clear demo --all` removes references without purging broker leases.

## Use from agents

Agents can run `teamy-discord` as a tool and consume its JSON results. With the
default Windows memory provider, reads follow this path:

```text
agent -> teamy-discord -> teamy-broker (credential approval and lease)
                      -> Discord API (read requests) -> JSON results to agent
```

Live `guild`, `channel`, `thread`, `user`, `message`, and `attachment` list commands
request a lease through the Rust SDK, then call Discord through Serenity. Online
`watch daemon serve` uses the same credential path, with Gateway wake hints and
REST reads. Help, profiles, plans, offline serving, and local watcher consumers
do not acquire credentials. Archive sync and invite use explicit tokens.

The broker request identifies the account/item, application, purpose, and
destination. Approval grants a fixed 15-minute lease. An identical unexpired
request can reuse it without extending its deadline. Denial, incompatibility, or
expiry fails without selecting another credential source. The CLI checks leases
before and after requests and while they are pending; live watcher expiry stops
the whole daemon.

Token bytes pass over local IPC into CLI process memory, without being printed in
results or saved in profiles or a token cache. The SDK zeroizes its owned buffer
on drop, but Serenity copies authorization headers. Expiry cannot guarantee
erasure of every copy or recall requests already sent to Discord.

Ad hoc agent posting used a separate worker: direct broker IPC acquisition,
followed by Discord REST calls. It did not use either CLI for acquisition.
This CLI has no send, edit, or delete commands.

The broker supplies credentials; it does not proxy Discord requests or enforce
routes and message bodies. Approval metadata does not reduce the bot's Discord
permissions. Profiles are defaults, not authorization. The CLI enforces its read
policy and target checks; a posting worker must enforce its approved scope,
content, and lease independently.

## Read and filter

Put group options such as `--profile`, `--plan`, and `--read-policy` before `list`.
Put IDs, limits, and filters after it. These examples use the saved `demo` profile
and perform live reads:

```powershell
teamy-discord guild --profile demo list
teamy-discord channel --profile demo list
teamy-discord thread --profile demo list
teamy-discord user --profile demo list --limit 50
teamy-discord message --profile demo list --thread-id 200000000000000002 --before 2026-01-01T00:00:00Z --limit 20
teamy-discord attachment --profile demo list --limit 20
```

Thread listing returns active threads. Message listing returns one page, default
100 messages, with `--limit` clamped to 1 through 100. Attachment listing inspects
that many messages and returns attachment metadata; archive sync downloads files.

Find explicit mentions of the authenticated bot within a bounded recent window:

```powershell
teamy-discord message --profile demo list --mentions me --limit 10 --scan-limit 300
teamy-discord message --profile demo --plan list --mentions 300000000000000001 --limit 10 --scan-limit 100
```

`me` means the authenticated bot, not its owner. A numeric user ID selects that
user. Matching uses Discord's structured user mentions, excluding plain text,
role mentions, and everyone mentions unless the user also appears explicitly.
`--limit` caps matches; `--scan-limit` caps fetched messages (default 100, maximum
1000) and requires `--mentions`. An empty result covers only the examined visible
history.

Unfiltered results are JSON arrays. Mention-filtered results are a JSON object
with `messages`, counts (`fetched`, `scanned`, `returned`), `stop_reason`, and
`next_before`, plus the selected identity and limits. `next_before` is a message
ID cursor; the CLI's `--before` accepts an RFC3339 timestamp, not that cursor.

## Archive and search saved data

Choose a destination, then sync. Unlike live queries, archive sync currently
requires one explicit token input, normally `TEAMY_DISCORD_BOT_TOKEN` injected
into the process by your secret manager. It does not use broker profile defaults.
Do not put a token in a command line, profile, or checked-in file.

```powershell
teamy-discord archive output-dir set ./discord-archive
teamy-discord archive output-dir show
teamy-discord archive sync
teamy-discord archive sync messages
teamy-discord archive sync members
```

`archive sync` runs both phases. `messages` saves messages and attachments;
`members` saves user records and avatars for authors found in archived messages,
not a complete member roster. Sync covers all bot-visible guilds and supported
channels, including discovered active threads. Profiles and query read policies
do not restrict it. Sync reads Discord and writes local files; it does not promise
complete archived-thread coverage or tracking of historical edits and deletions.

The archive contains `guilds/<guild-id>/channels/<channel-id>/messages/`, nested
thread directories, and JSON metadata. Message files keep Discord payloads in a
serialized `raw_json` string; parse that string for structured message access.
Attachment metadata lives under `attachments/by-id/`, with deduplicated blobs
under `attachments/blobs/sha256/`.
Checkpoints live in the application cache, keyed by output-root path; a missing
checkpoint can be reconstructed from existing output. Preview reconstruction
without credentials, network access, or directory/checkpoint writes:

```powershell
teamy-discord archive sync --output-dir ./discord-archive checkpoint restore --dry-run
```

There is no built-in full-text search command. With
[ripgrep](https://github.com/BurntSushi/ripgrep) installed, search saved JSON text:

```powershell
rg --fixed-strings --glob '*.json' 'release checklist' ./discord-archive/guilds
```

This searches local serialized data, including metadata; it does not query Discord.

## Mention workflows on Windows

The watcher captures explicit mentions of one bot by one human author in one
channel. Preview the scope first, using an absolute state directory:

```powershell
teamy-discord watch daemon serve --profile demo --state-dir C:/discord-watch/demo --author-id 300000000000000001 --bot-id 400000000000000001 --plan
```

Remove `--plan` to start the foreground daemon under broker approval. Initial
capture starts after the newest visible message, ignoring existing history;
`--backfill-after-id` explicitly opts into initial history. Keep it running and use
another terminal for local consumers:

```powershell
teamy-discord watch consumer create --state-dir C:/discord-watch/demo --consumer review --from now
teamy-discord watch next --state-dir C:/discord-watch/demo --consumer review
teamy-discord watch claim inspect --state-dir C:/discord-watch/demo --consumer review
```

`watch next` waits for one JSON delivery and creates a durable claim. After handling
it, use `watch outbox ack` with its exact event ID, delivery ID, and a nonsecret
receipt. Inspect an uncertain claim before explicitly replaying it. Consumers and
acknowledgments change local workflow state; they do not send Discord replies.
All watcher commands support `--plan` without IPC or state changes.

Lease expiry stops the daemon, including IPC. An explicit offline restart can
serve retained captures and claims without credentials. See the
[watcher guide](docs/mentions-watcher.md) for acknowledgments, recovery, and result
codes.

## Configuration and output

Use `home show` and `cache show` to locate application storage. Platform directory
names remain `teamy-discord-archive`; upgrading does not move existing data.
Archive destination precedence is `archive sync --output-dir`, then the
environment, then the preference saved by `archive output-dir set`.

| Environment variable | Purpose |
| --- | --- |
| `TEAMY_DISCORD_HOME_DIR` | Profiles, read policy, and saved output preference |
| `TEAMY_DISCORD_CACHE_DIR` | Cache and sync checkpoints |
| `TEAMY_DISCORD_OUTPUT_DIR` | Archive destination override |
| `TEAMY_DISCORD_BROKER_ACCOUNT` | Nonsecret 1Password account selector |
| `TEAMY_DISCORD_BROKER_SECRET_REF` | Nonsecret `op://vault/item/field` reference |
| `TEAMY_DISCORD_BROKER_EXECUTABLE` | Broker executable override |
| `TEAMY_DISCORD_CREDENTIAL_PROVIDER` | Explicit provider selection |
| `TEAMY_DISCORD_CREDENTIAL_TARGET` | Existing protected credential target |
| `TEAMY_DISCORD_CREDENTIAL_LEASE_ID` | Existing broker lease UUID |
| `TEAMY_DISCORD_BOT_TOKEN` | Explicit process token, required for sync/invite |
| `RUST_LOG` | Stderr filter when no explicit filter or `--debug` is supplied |

Broker fields resolve independently from an explicit CLI/environment input, then
the selected profile. Supplying a credential setting through both CLI and
environment is an error even when equal. Direct tokens conflict with explicit
provider/broker settings. Linux live operations need a process token; direct
tokens have no broker lease enforcement.

`windows-credential-manager` and `well-known-file` are explicit Windows providers
for existing broker leases, selected with `--credential-provider` and paired with
`--credential-target` and `--credential-lease-id`. They do not acquire a new lease
or fall back to another provider.

Legacy `TEAMY_DISCORD_ARCHIVE_HOME_DIR`, `TEAMY_DISCORD_ARCHIVE_CACHE_DIR`, and
`TEAMY_DISCORD_ARCHIVE_OUTPUT_DIR` remain fallbacks for blank/missing new values.
The legacy token variable `TEAMY_DISCORD_ARCHIVE_DISCORD_BOT_TOKEN` remains an
explicit input and conflicts with the new token variable. Saved plaintext tokens
are never read. The old `bot-token` commands and `--token` flag are removed; an
existing `discord-bot-token.txt` triggers a migration warning and is left intact.

Command results go to stdout; diagnostics go to stderr. Preserve Discord IDs as
64-bit integers when consuming JSON. `--log-filter`, `--log-format text|ndjson`,
and the optional `--log-file` control diagnostics independently of command data.
`--log-file` takes a filename and appends; no output-format switch is implemented.
See [logging options](docs/logging-progress.md).

### Local read policy

Read groups load `read-policy.json` from app home when present, or an explicit
`--read-policy` file. A policy restricts reads independently of profiles and
Discord permissions. Without one, reads use the bot's visible scope. For example:

```json
{
  "version": 1,
  "guild-ids": [100000000000000001],
  "channel-ids": [200000000000000001],
  "include-child-threads": true,
  "allow-user-list": false
}
```

Empty allowlists allow nothing. A listed parent allows its child threads only
with `include-child-threads`; individual thread IDs can be listed directly.
Listing users requires `allow-user-list`. A category ID does not authorize its
ordinary channels. Invalid policies fail explicitly. Configure a protected policy
for unattended readers; profiles supply defaults without granting access.

## Command discovery and development

```powershell
teamy-discord --help
teamy-discord message --help
teamy-discord message list --help
teamy-discord archive sync --help
teamy-discord watch --help
```

Use help at each level for that command's options and children. Help, version,
and shell completion generation do not acquire credentials or contact Discord.
`invite --no-open` fetches the bot's invite URL using an explicit token; `invite`
also opens it in a browser, where server installation is a separate user action.

For local development:

```powershell
cargo test --locked
```

`check-all.ps1` also runs nightly formatting, Clippy, a build, and Tracey validation;
it requires those tools and can format source files. See the
[CLI specification](docs/spec/product/cli.md),
[archive specification](docs/spec/product/archive.md), and
[parser diagnostics](docs/parser-diagnostics.md) for implementation details.

<!-- repo[impl license.present] -->
<!-- repo[impl readme.source-links] -->
Licensed under [MPL-2.0](LICENSE). Source and issues:
[TeamDman/teamy-discord](https://github.com/TeamDman/teamy-discord).
