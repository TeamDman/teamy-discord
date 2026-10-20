#[derive(Debug, facet::Facet)]
#[facet(rename_all = "kebab-case")]
pub struct MessageListPlan {
    pub query: super::query_plan::QueryPlan,
    pub mentions: String,
    pub limit: u8,
    pub scan_limit: u16,
    pub identity_resolution: &'static str,
}
