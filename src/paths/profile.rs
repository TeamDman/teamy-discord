use facet::Facet;
/// Nonsecret query defaults and optional broker item addresses. Grants no access.
#[derive(Facet, Clone, Debug, PartialEq, Eq)]
#[facet(rename_all = "kebab-case")]
pub struct Profile {
    pub name: String,
    pub guild_id: u64,
    pub channel_id: Option<u64>,
    #[facet(default)]
    pub broker: Option<super::BrokerReference>,
}
