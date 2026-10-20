#[derive(Clone, Copy, Debug, Eq, PartialEq, facet::Facet)]
#[repr(u8)]
pub enum ConsumerOperation {
    Create,
    Claim,
    Inspect,
    Replay,
    Acknowledge,
}
