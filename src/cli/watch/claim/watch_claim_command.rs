use arbitrary::Arbitrary;
use facet::Facet;

/// Inspect or explicitly replay a pending delivery.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum WatchClaimCommand {
    /// Inspect claim metadata without replaying private message payload.
    Inspect(super::inspect::WatchClaimInspectArgs),
    /// Replay the exact in-flight delivery; does not acknowledge it.
    Replay(super::replay::WatchClaimReplayArgs),
}
