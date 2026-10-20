# Linux support

The CLI supports Linux x86_64 for profiles, local archive operations, plans, and
Discord operations with an explicitly injected token. Windows keeps the
`teamy-broker` SDK and named-pipe transport; broker authentication and watcher IPC
are unavailable on Linux.

## Credentials

Inject `TEAMY_DISCORD_BOT_TOKEN` into the process through the application's
approved secret-delivery mechanism. Do not put it in an image, build argument,
profile, command line, or log. Do not combine it with explicit provider/broker
metadata. Saved profile broker references do not override a direct token.

Selecting `teamy-broker`, `windows-credential-manager`, or `well-known-file` on
Linux fails with an unsupported-transport diagnostic. It does not launch a broker,
read a saved token, or choose another source. Profiles can retain nonsecret broker
references for use on Windows.

Direct credentials have no broker expiry enforcement. The launching application
owns access lifetime and revocation. Serenity copies authorization headers
internally; this CLI provides no persistent secret store or enrollment flow.

## Build and runtime

Keep the sibling `teamy-broker/sdk` source available for Cargo dependency
resolution even when building for Linux. The manifest currently uses a path
dependency. Build from the repository root:

```sh
cargo test --locked
cargo build --locked --release
```

The GNU Linux build needs a compatible glibc runtime and its dynamically linked
libraries. Check the produced executable with `ldd target/release/teamy-discord`
in the deployment environment. Include CA certificates for HTTPS requests.
When building or testing in a container, run fixtures without Discord credentials;
dependency fetching may need build-network access.

Install the executable at an application-owned location or use `cargo install
--locked --path . --bin teamy-discord`. Give the runtime user writable home,
cache, and archive directories through `TEAMY_DISCORD_HOME_DIR`,
`TEAMY_DISCORD_CACHE_DIR`, and `TEAMY_DISCORD_OUTPUT_DIR` as appropriate.
Headless environments should use `home show` and `cache show`; the corresponding
`open` commands require an OS opener and desktop session.

Check help and a credential-free plan before configuring live reads. These IDs
are synthetic:

```sh
teamy-discord --help
teamy-discord home show
teamy-discord message --plan list --guild-id 100000000000000001 --channel-id 200000000000000001 --limit 3
```

Live operations also require the bot's intended Discord permissions and allowed
HTTPS egress. See the [README](../README.md) for profiles, read policies, and the
archive format.
