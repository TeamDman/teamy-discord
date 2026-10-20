# CLI

This specification covers the current user-facing command line behavior exposed by `teamy-discord`.

## Command Surface

cli[command.surface.core]
The CLI must expose `cache`, `home`, `invite`, `channel`, `message`, `thread`, `user`, `guild`, `attachment`, `profile`, `archive`, and `watch`. There is one teamy-discord executable and no compatibility routes.

cli[command.surface.invite]
The CLI must expose a top-level `invite` command.

cli[command.surface.cache]
The `cache` command group must expose the `show`, `open`, and `clean` subcommands.

cli[command.surface.home]
The `home` command group must expose the `show` and `open` subcommands.

cli[command.surface.output-dir]
The `archive output-dir` command group must expose the `show`, `open`, and `set` subcommands.

Guild, channel, thread, message, attachment and user are root query groups.

cli[command.surface.live-guild]
The `guild` command group must expose the `list` subcommand.

cli[command.surface.live-channel]
The `channel` command group must expose the `list` subcommand.

cli[command.surface.live-thread]
The `thread` command group must expose the `list` subcommand.

cli[command.surface.live-message]
The `message` command group must expose the `list` subcommand.

cli[command.surface.live-attachment]
The `attachment` command group must expose the `list` subcommand.

cli[command.surface.live-user]
The `user` command group must expose the `list` subcommand.

cli[command.surface.sync]
The `archive sync` command must run resumable archival and expose the `messages`, `members`, and `checkpoint` subcommand groups.

cli[command.surface.sync-messages]
The `archive sync messages` command must expose the resumable message and attachment archival stage on its own.

cli[command.surface.sync-members]
The `archive sync members` command must expose the user-record and avatar archival stage for archived message authors on its own.

cli[command.surface.sync-checkpoint]
The `archive sync checkpoint` command group must expose the `restore` subcommand.

## Parser Model

cli[parser.args-consistent]
The structured CLI model must serialize to command line arguments consistently for parse-safe values.

cli[parser.roundtrip]
The structured CLI model must roundtrip through argument serialization and parsing for parse-safe values.

## Path Resolution

cli[path.app-home.env-overrides-platform]
If `TEAMY_DISCORD_HOME_DIR` is set to a non-empty value, it must take precedence over the platform-derived application home directory.

cli[path.cache.env-overrides-platform]
If `TEAMY_DISCORD_CACHE_DIR` is set to a non-empty value, it must take precedence over the platform-derived cache directory.

cli[auth.live-token.env]
If `TEAMY_DISCORD_BOT_TOKEN` is set to a non-empty value, `invite`, archive sync, and live query commands may use it as the Discord bot token.

cli[auth.live-token.ambiguous-inputs-rejected]
Multiple explicit token inputs, CLI/environment provider inputs, or token-plus-provider metadata must be rejected even when equal. Invalid or blank explicit choices must never select another source.

cli[auth.live-token.broker-memory-default]
On Windows, live query commands default to a broker memory lease with configured account and item-field metadata, native approval and safe start only when the broker endpoint is absent. They must never silently read a saved plaintext preference. Existing incompatible brokers and denial must fail closed. Linux requires an explicit direct token and must report broker providers as unsupported.

cli[auth.invite-token-resolution]
The legacy invite command requires exactly one explicit CLI/environment token; it must reject ambiguity and must not read the saved plaintext preference. Broker integration for invite is separate work.

cli[invite.prints-url]
The `invite` command must print the resolved Discord bot invite URL to stdout.

cli[invite.opens-browser-by-default]
The `invite` command must open the resolved Discord bot invite URL in the default browser unless `--no-open` is provided.

cli[auth.legacy-token-storage.removed]
The bot-token command group is removed without aliases. Credentials are never saved. One-off input uses --bot-token or one explicit token environment variable; --token is not an alias. Metadata-only detection of discord-bot-token.txt emits safe migration guidance even with logging disabled and never reads or changes its contents.

cli[path.output-dir.env-overrides-preference]
If `TEAMY_DISCORD_OUTPUT_DIR` is set to a non-empty value, it must take precedence over the persisted output directory preference.

cli[path.output-dir.command-line-overrides-env]
If `archive sync --output-dir <path>` is provided, it must take precedence over `TEAMY_DISCORD_OUTPUT_DIR`.

cli[path.output-dir.show-resolves-effective]
The `archive output-dir show` command must report the effective resolved output directory.

cli[path.output-dir.open-resolves-effective]
The `archive output-dir open` command must resolve the same effective output directory that `archive output-dir show` reports.

cli[path.output-dir.set-persists-default]
The `archive output-dir set <path>` command must persist the default output directory in the application home directory.

cli[sync.requires-output-dir]
The `archive sync` command must resolve the output directory from the command line, then the environment, then the saved preference, and fail with a configuration error when none is configured.

cli[sync.requires-token]
Archive synchronization must require exactly one explicit command-line or environment bot-token input and reject missing, blank, or conflicting inputs. Local checkpoint restoration must not require a token.

cli[sync.checkpoint.restore.dry-run]
The `archive sync checkpoint restore --dry-run` command must reconstruct a candidate checkpoint from archived output and print a comparison with any existing checkpoint without writing the checkpoint file.

cli[live.message.before-flag]
The `message list` command must support a `--before <rfc3339>` flag to query messages before a cursor timestamp.

cli[live.message.target-selection]
The `message list` and `attachment list` commands must reject conflicting `--channel-id` and `--thread-id`, and require a target from one explicit ID or the selected profile default.

## Profiles and local read policy

Profiles contain nonsecret named guild/channel defaults. Explicit profile selection
overrides the active profile; explicit IDs override profile defaults. Changing the
guild cannot inherit a channel from a different guild. Profiles confer no access.
Profile commands never read credentials and perform writes only when explicitly
invoked. Invalid/duplicate names, zero IDs, corrupt files and unknown active names
fail clearly. An exclusive read-modify-write transaction refreshes the document under its guard, preserving other committed profile/active-selection edits; competing writes fail explicitly.

Read groups accept plan/profile/archive-root/read-policy/bot-token options. Plan mode
never resolves credentials or initializes HTTP. Archived channel/thread metadata
provides historical evidence only; unknown membership remains pending live checks.
Independent read policies restrict guilds and channel IDs, with explicit child-thread
inheritance and user-list permission. Missing explicit policy paths fail closed.
Absent policy preserves existing interactive bot-visible reads, not agent authority.

Live execution checks target membership before reading history. Query policy and
profiles do not scope archive sync, which retains all bot-visible guild behavior.
Physical app home/cache identities and archive/checkpoint locations remain unchanged.
Nonblank new noncredential environment names take precedence over legacy names; no data is copied. Token aliases are explicit inputs and conflict when both are present.
No credentials open the removed saved-token file; secret Debug is redacted.

See README.md for commands and limitations. The CLI has no full-history content
search, draft, or send command.

The active profile is optional. profile clear removes only its selection, never the named profiles. An absent active profile cannot select the first saved entry. Explicit IDs work without a profile; commands requiring missing defaults fail with an actionable flag suggestion before credentials/network.

## Bounded user-mention filter

Message list supports --mentions me or a nonzero numeric user ID. Me resolves the authenticated bot with users/@me and never uses the owner ID. Matching uses the structured mentions array. Limit bounds returned matches; scan-limit bounds fetched recent messages (default 100, maximum 1000) with backward pages of at most 100. Filtered results expose fetched/scanned counts, stop reason and cursor; empty results cannot claim absence outside the inspected visible scope. Plain unfiltered array output is preserved, and planning remains credential-free. Broker-backed HTTP requests obey the fixed lease; explicit direct tokens have no broker lease enforcement.

Help before the -- terminator takes precedence over argument errors and selects only the verified command prefix. Unknown-option diagnostics include safe schema context, reviewed public labels where available and the exact help route; all payloads remain redacted.
