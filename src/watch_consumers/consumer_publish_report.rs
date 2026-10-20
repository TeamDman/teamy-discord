#[derive(Clone, Debug, Eq, PartialEq, facet::Facet)]
pub struct ConsumerPublishReport {
    pub appended: u64,
    pub last_sequence: u64,
}
