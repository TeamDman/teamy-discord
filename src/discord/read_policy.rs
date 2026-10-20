use eyre::Context;
use eyre::Result;
use eyre::bail;
use facet::Facet;
use std::path::Path;
/// Independent local restriction, not a source of Discord permissions.
#[derive(Facet, Clone, Debug, PartialEq, Eq)]
#[facet(rename_all = "kebab-case")]
pub struct ReadPolicy {
    pub version: u32,
    pub guild_ids: Vec<u64>,
    pub channel_ids: Vec<u64>,
    pub include_child_threads: bool,
    pub allow_user_list: bool,
}
impl ReadPolicy {
    /// # Errors
    /// Returns an error for invalid configuration, rejected scope or a failed operation.
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).wrap_err("Failed to read local read policy")?;
        let value: Self = facet_json::from_str(&text).wrap_err("Invalid read policy")?;
        if value.version != 1 || value.guild_ids.contains(&0) || value.channel_ids.contains(&0) {
            bail!("Invalid read policy version or zero IDs");
        }
        Ok(value)
    }
    /// # Errors
    /// Returns an error for invalid configuration, rejected scope or a failed operation.
    pub fn check_guild(&self, guild: u64) -> Result<()> {
        if !self.guild_ids.contains(&guild) {
            bail!("Guild {guild} is outside the local read policy");
        }
        Ok(())
    }
    #[must_use]
    pub fn allows_channel(&self, channel: u64, parent: Option<u64>, is_thread: bool) -> bool {
        self.channel_ids.contains(&channel)
            || (is_thread
                && self.include_child_threads
                && parent.is_some_and(|id| self.channel_ids.contains(&id)))
    }
    /// # Errors
    /// Returns an error for invalid configuration, rejected scope or a failed operation.
    pub fn check_channel(&self, channel: u64, parent: Option<u64>, is_thread: bool) -> Result<()> {
        if !self.allows_channel(channel, parent, is_thread) {
            bail!("Channel/thread {channel} is outside the local read policy");
        }
        Ok(())
    }
}
