#[derive(Clone, Debug, Eq, PartialEq, facet::Facet)]
pub struct ConsumerProvenance {
    pub tool: String,
    pub tool_version: String,
    pub source_repository: String,
    pub source_revision: String,
    pub created_unix_ms: u64,
    pub updated_unix_ms: u64,
}

impl ConsumerProvenance {
    pub(super) fn new() -> eyre::Result<Self> {
        let now = Self::now()?;
        Ok(Self {
            tool: "teamy-discord".to_owned(),
            tool_version: env!("CARGO_PKG_VERSION").to_owned(),
            source_repository: env!("CARGO_PKG_REPOSITORY").to_owned(),
            source_revision: option_env!("GIT_REVISION").unwrap_or("unknown").to_owned(),
            created_unix_ms: now,
            updated_unix_ms: now,
        })
    }

    pub(super) fn now() -> eyre::Result<u64> {
        Ok(u64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis(),
        )?)
    }
}
