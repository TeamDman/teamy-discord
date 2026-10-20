use facet::Facet;
#[derive(Facet, Debug, PartialEq, Eq)]
#[facet(rename_all = "kebab-case")]
pub struct QueryPlan {
    pub operation: String,
    pub profile: Option<String>,
    pub guild_id: Option<u64>,
    pub channel_id: Option<u64>,
    pub parent_channel_id: Option<u64>,
    pub membership_evidence: String,
    pub local_read_policy: String,
    pub discord_permissions_verified: bool,
    pub thread_requested: bool,
    pub profile_broker_configured: bool,
}
