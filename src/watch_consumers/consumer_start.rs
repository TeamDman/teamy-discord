#[derive(Clone, Copy, Debug, Eq, PartialEq, facet::Facet)]
#[repr(u8)]
pub enum ConsumerStart {
    Now,
    Oldest,
}
