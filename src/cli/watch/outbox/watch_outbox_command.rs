use arbitrary::Arbitrary;
use facet::Facet;

/// Acknowledge externally handled deliveries through local IPC.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum WatchOutboxCommand {
    /// Acknowledge the exact delivery after external handling has produced a receipt.
    Ack(super::ack::WatchOutboxAckArgs),
}
