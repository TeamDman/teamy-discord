use arbitrary::Arbitrary;
use facet::Facet;

/// Manage the capture daemon for one state directory.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
#[expect(
    clippy::large_enum_variant,
    reason = "Figue 2.0.1 only flattens direct struct subcommands; boxing would change the CLI schema"
)]
pub enum WatchDaemonCommand {
    /// Capture the fixed approved Discord scope, or explicitly serve an offline fixture.
    Serve(super::serve::WatchDaemonServeArgs),
    /// Read daemon status over local IPC.
    Status(super::status::WatchDaemonStatusArgs),
    /// Request graceful local daemon shutdown; preserve committed state and pending claims.
    Stop(super::stop::WatchDaemonStopArgs),
}
