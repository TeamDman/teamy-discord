use arbitrary::Arbitrary;
use facet::Facet;

/// Local watcher operations; only daemon serve may contact Discord.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
#[expect(
    clippy::large_enum_variant,
    reason = "Figue 2.0.1 only flattens direct struct subcommands; boxing would change the CLI schema"
)]
pub enum WatchCommand {
    /// Serve, inspect or stop the single local capture daemon.
    Daemon(super::daemon::WatchDaemonArgs),
    /// Wait locally for one delivery; writes exactly one JSON result, never an event stream.
    Next(super::next::WatchNextArgs),
    /// Create a named consumer with an independent durable cursor.
    Consumer(super::consumer::WatchConsumerArgs),
    /// Inspect an in-flight claim or explicitly replay its exact delivery.
    Claim(super::claim::WatchClaimArgs),
    /// Record a receipt for a delivery after its external handling completes.
    Outbox(super::outbox::WatchOutboxArgs),
}
