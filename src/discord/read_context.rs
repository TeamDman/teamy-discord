//! Local planning is credential-free. Live membership checks occur only on execution.
pub use super::query_plan::QueryPlan;
pub use super::read_options::ReadOptions;
pub use super::read_policy::ReadPolicy;
use crate::paths::AppHome;
use crate::paths::Profile;
use crate::paths::Profiles;
use eyre::Context;
use eyre::Result;
use eyre::bail;
use serenity::all::Channel;
use serenity::all::ChannelId;
use serenity::all::ChannelType;
use serenity::all::GuildId;
use serenity::all::Http;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug)]
pub struct ReadContext {
    pub options: ReadOptions,
    pub profile: Option<Profile>,
    pub policy: Option<ReadPolicy>,
    pub archive_root: Option<PathBuf>,
}
impl ReadContext {
    /// # Errors
    /// Returns an error for invalid configuration, rejected scope or a failed operation.
    pub fn load(options: ReadOptions) -> Result<Self> {
        Self::load_with(options, &crate::paths::APP_HOME)
    }
    /// # Errors
    /// Returns an error for invalid configuration, rejected scope or a failed operation.
    pub fn load_with(options: ReadOptions, home: &AppHome) -> Result<Self> {
        let profiles = Profiles::load(home)?;
        let profile = profiles.selected(options.profile.as_deref())?.cloned();
        let policy_path = options
            .read_policy
            .as_ref()
            .map_or_else(|| home.file_path("read-policy.json"), PathBuf::from);
        let policy = if options.read_policy.is_some() {
            Some(ReadPolicy::load(&policy_path)?)
        } else {
            load_default_policy(&policy_path, std::fs::symlink_metadata(&policy_path))?
        };
        let archive_root = if let Some(root) = &options.archive_root {
            Some(PathBuf::from(root))
        } else if let Some(root) = crate::paths::load_output_dir_from_environment() {
            Some(root)
        } else {
            crate::paths::load_output_dir_preference(home)?
        };
        Ok(Self {
            options,
            profile,
            policy,
            archive_root,
        })
    }
    /// # Errors
    /// Returns an error for invalid configuration, rejected scope or a failed operation.
    pub fn guild(&self, explicit: Option<u64>) -> Result<u64> {
        let guild = explicit
            .or(self.profile.as_ref().map(|p| p.guild_id))
            .ok_or(super::configuration_error::ConfigurationError::MissingGuild)?;
        if guild == 0 {
            bail!("Guild ID must be nonzero");
        }
        if let Some(policy) = &self.policy {
            policy.check_guild(guild)?;
        }
        Ok(guild)
    }
    /// # Errors
    /// Returns an error for invalid configuration, rejected scope or a failed operation.
    pub fn guild_plan(&self, operation: &str, guild: Option<u64>) -> Result<QueryPlan> {
        let guild = self.guild(guild)?;
        if operation == "user.list" && self.policy.as_ref().is_some_and(|p| !p.allow_user_list) {
            bail!("Guild user listing is disabled by the local read policy");
        }
        Ok(self.record(
            operation,
            Some(guild),
            None,
            None,
            "requires-live-membership-check",
        ))
    }
    /// # Errors
    /// Returns an error for invalid configuration, rejected scope or a failed operation.
    pub fn message_plan(
        &self,
        operation: &str,
        guild: Option<u64>,
        channel: Option<u64>,
        thread: Option<u64>,
    ) -> Result<QueryPlan> {
        let resolved_guild = self.guild(guild)?;
        if channel.is_some() && thread.is_some() {
            bail!("Pass --channel-id or --thread-id, not both");
        }
        let inherited = channel.is_none() && thread.is_none();
        if inherited
            && self
                .profile
                .as_ref()
                .is_some_and(|p| p.guild_id != resolved_guild && p.channel_id.is_some())
        {
            bail!(
                "Changing the profile guild requires an explicit channel/thread; its channel default belongs to another guild"
            );
        }
        let target = channel
            .or(thread)
            .or_else(|| self.profile.as_ref().and_then(|p| p.channel_id))
            .ok_or(super::configuration_error::ConfigurationError::MissingChannel)?;
        if target == 0 {
            bail!("Channel/thread ID must be nonzero");
        }
        let metadata = self
            .archive_root
            .as_ref()
            .map(|root| archived_target(root, resolved_guild, target))
            .transpose()?
            .flatten();
        let (parent, evidence) = if let Some(meta) = metadata {
            validate_membership(resolved_guild, target, meta.guild_id, meta.channel_id)?;
            if thread.is_some() && !meta.is_thread {
                bail!("--thread-id identifies a non-thread in archive metadata");
            }
            if let Some(policy) = &self.policy {
                policy.check_channel(target, meta.parent_id, meta.is_thread)?;
            }
            (meta.parent_id, "archive-metadata-only; live-check-required")
        } else {
            (None, "requires-live-membership-check")
        };
        // An exact-target rejection can be final without metadata. Parent inheritance requires evidence.
        if metadata.is_none()
            && let Some(policy) = &self.policy
            && !policy.channel_ids.contains(&target)
            && !policy.include_child_threads
        {
            bail!("Channel/thread {target} is outside the local read policy");
        }
        let mut plan = self.record(
            operation,
            Some(resolved_guild),
            Some(target),
            parent,
            evidence,
        );
        plan.thread_requested = thread.is_some();
        Ok(plan)
    }
    fn record(
        &self,
        operation: &str,
        guild: Option<u64>,
        channel: Option<u64>,
        parent: Option<u64>,
        evidence: &str,
    ) -> QueryPlan {
        QueryPlan {
            operation: operation.into(),
            profile: self.profile.as_ref().map(|p| p.name.clone()),
            guild_id: guild,
            channel_id: channel,
            parent_channel_id: parent,
            membership_evidence: evidence.into(),
            local_read_policy: if self.policy.is_some() {
                "configured; execution rechecks target".into()
            } else {
                "unconfigured; profile grants no access".into()
            },
            discord_permissions_verified: false,
            thread_requested: false,
            profile_broker_configured: self
                .profile
                .as_ref()
                .and_then(|p| p.broker.as_ref())
                .is_some_and(|b| b.account.is_some() && b.secret_ref.is_some()),
        }
    }
    /// # Errors
    /// Returns an error for invalid configuration, rejected scope or a failed operation.
    pub fn emit_plan(&self, plan: &QueryPlan) -> Result<bool> {
        if self.options.plan {
            crate::json_stdout::print_facet_json(plan)?;
            return Ok(true);
        }
        Ok(false)
    }
    /// # Errors
    /// Returns an error for invalid configuration, rejected scope or a failed operation.
    pub async fn http(
        &self,
        operation: &str,
        guild: Option<u64>,
        channel: Option<u64>,
    ) -> Result<super::leased_http::LeasedHttp> {
        self.options
            .credential
            .connect_with_profile(
                self.options.bot_token.as_deref(),
                operation,
                guild,
                channel,
                self.profile.as_ref().and_then(|p| p.broker.as_ref()),
            )
            .await
    }
    /// # Errors
    /// Returns an error for invalid configuration, rejected scope or a failed operation.
    pub async fn check_live_guild(&self, http: &Http, guild: u64) -> Result<()> {
        http.get_guild(GuildId::new(guild))
            .await
            .wrap_err("Failed to verify bot guild access")?;
        Ok(())
    }
    /// # Errors
    /// Returns an error for invalid configuration, rejected scope or a failed operation.
    pub async fn check_live_target(&self, http: &Http, plan: &QueryPlan) -> Result<()> {
        let channel_id = plan
            .channel_id
            .ok_or_else(|| eyre::eyre!("Missing target"))?;
        let guild_id = plan.guild_id.ok_or_else(|| eyre::eyre!("Missing guild"))?;
        let channel = http
            .get_channel(ChannelId::new(channel_id))
            .await
            .wrap_err("Failed to verify channel membership")?;
        let Channel::Guild(channel) = channel else {
            bail!("Direct messages are outside guild queries");
        };
        validate_membership(
            guild_id,
            channel_id,
            channel.guild_id.get(),
            channel.id.get(),
        )?;
        if plan.thread_requested && !is_thread(channel.kind) {
            bail!("--thread-id identifies a non-thread");
        }
        if let Some(policy) = &self.policy {
            policy.check_channel(
                channel_id,
                channel.parent_id.map(serenity::all::ChannelId::get),
                is_thread(channel.kind),
            )?;
        }
        Ok(())
    }
}

#[must_use]
pub fn is_thread(kind: ChannelType) -> bool {
    matches!(
        kind,
        ChannelType::NewsThread | ChannelType::PublicThread | ChannelType::PrivateThread
    )
}
/// # Errors
/// Returns an error for invalid configuration, rejected scope or a failed operation.
pub fn validate_membership(
    expected_guild: u64,
    expected_channel: u64,
    guild: u64,
    channel: u64,
) -> Result<()> {
    if expected_guild != guild || expected_channel != channel {
        bail!("Channel metadata does not match the requested guild/channel");
    }
    Ok(())
}
fn load_default_policy(
    path: &Path,
    metadata: std::io::Result<std::fs::Metadata>,
) -> Result<Option<ReadPolicy>> {
    match metadata {
        Ok(_) => ReadPolicy::load(path).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).wrap_err("Failed to inspect default read policy"),
    }
}

use super::archived_target::ArchivedTarget;
fn json_id(value: &serde_json::Value) -> Option<u64> {
    value.as_u64().or_else(|| value.as_str()?.parse().ok())
}
fn read_target(path: &Path) -> Result<ArchivedTarget> {
    let text =
        std::fs::read_to_string(path).wrap_err("Failed to read archived channel metadata")?;
    let value: serde_json::Value =
        serde_json::from_str(&text).wrap_err("Invalid archived channel metadata")?;
    Ok(ArchivedTarget {
        guild_id: json_id(&value["guild_id"])
            .ok_or_else(|| eyre::eyre!("Missing guild_id in channel metadata"))?,
        channel_id: json_id(&value["id"])
            .ok_or_else(|| eyre::eyre!("Missing id in channel metadata"))?,
        parent_id: json_id(&value["parent_id"]),
        is_thread: matches!(value["type"].as_u64(), Some(10..=12)),
    })
}
fn archived_target(root: &Path, guild: u64, channel: u64) -> Result<Option<ArchivedTarget>> {
    let guild_root = root.join("guilds").join(guild.to_string());
    let channels = guild_root.join("channels");
    let direct = channels.join(channel.to_string()).join("channel.json");
    if direct.exists() {
        return read_target(&direct).map(Some);
    }
    let orphan = guild_root
        .join("threads")
        .join(channel.to_string())
        .join("thread.json");
    if orphan.exists() {
        return read_target(&orphan).map(Some);
    }
    if channels.exists() {
        for entry in std::fs::read_dir(channels)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let parent = entry.file_name().to_string_lossy().parse::<u64>();
            let Ok(parent) = parent else {
                continue;
            };
            let candidate = entry
                .path()
                .join("threads")
                .join(channel.to_string())
                .join("thread.json");
            if candidate.exists() {
                let target = read_target(&candidate)?;
                if !target.is_thread || target.parent_id != Some(parent) {
                    bail!("Thread parent metadata disagrees with its archive location");
                }
                return Ok(Some(target));
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context() -> ReadContext {
        ReadContext {
            options: ReadOptions::default(),
            profile: Some(Profile {
                name: "sfm".into(),
                guild_id: 10,
                channel_id: Some(20),
                broker: None,
            }),
            policy: None,
            archive_root: None,
        }
    }
    #[test]
    fn explicit_targets_override_defaults_without_cross_guild_inheritance() {
        let context = context();
        assert_eq!(
            context
                .message_plan("message.list", None, None, None)
                .unwrap()
                .channel_id,
            Some(20)
        );
        let plan = context
            .message_plan("message.list", Some(30), None, Some(40))
            .unwrap();
        assert_eq!((plan.guild_id, plan.channel_id), (Some(30), Some(40)));
        assert!(!plan.discord_permissions_verified);
        let error = context
            .message_plan("message.list", Some(30), None, None)
            .unwrap_err();
        assert!(error.to_string().contains("explicit channel"));
        let error = context
            .message_plan("message.list", None, Some(20), Some(40))
            .unwrap_err();
        assert!(error.to_string().contains("not both"));
        let error = context.guild(Some(0)).unwrap_err();
        assert!(error.to_string().contains("nonzero"));
    }
    #[test]
    fn scope_does_not_follow_profile_and_thread_inheritance_is_explicit() {
        let policy = ReadPolicy {
            version: 1,
            guild_ids: vec![10],
            channel_ids: vec![20],
            include_child_threads: true,
            allow_user_list: false,
        };
        assert!(policy.check_guild(30).is_err());
        policy.check_channel(21, Some(20), true).unwrap();
        assert!(policy.check_channel(21, Some(20), false).is_err());
        assert!(policy.check_channel(21, Some(99), true).is_err());
        let mut context = context();
        context.policy = Some(policy);
        let error = context.guild_plan("user.list", None).unwrap_err();
        assert!(error.to_string().contains("disabled"));
        let error = context.guild_plan("channel.list", Some(30)).unwrap_err();
        assert!(error.to_string().contains("outside"));
        context.policy.as_mut().unwrap().include_child_threads = false;
        let error = context
            .message_plan("message.list", None, None, Some(21))
            .unwrap_err();
        assert!(error.to_string().contains("outside"));
    }
    #[test]
    fn offline_metadata_checks_membership_and_forum_parent_without_opening_secret_file() {
        let dir = tempfile::tempdir().unwrap();
        let home = AppHome(dir.path().join("home"));
        home.ensure_dir().unwrap();
        // A directory where a secret file would be makes any attempted secret read fail.
        std::fs::create_dir(home.file_path("discord-bot-token.txt")).unwrap();
        let root = dir.path().join("archive");
        let thread = root.join("guilds/10/channels/20/threads/21/thread.json");
        std::fs::create_dir_all(thread.parent().unwrap()).unwrap();
        std::fs::write(
            &thread,
            r#"{"id":"21","guild_id":"10","parent_id":"20","type":11}"#,
        )
        .unwrap();
        let mut context = ReadContext::load_with(
            ReadOptions {
                plan: true,
                archive_root: Some(root.to_string_lossy().into_owned()),
                ..ReadOptions::default()
            },
            &home,
        )
        .unwrap();
        context.policy = Some(ReadPolicy {
            version: 1,
            guild_ids: vec![10],
            channel_ids: vec![20],
            include_child_threads: true,
            allow_user_list: false,
        });
        let plan = context
            .message_plan("message.list", Some(10), None, Some(21))
            .unwrap();
        assert_eq!(plan.parent_channel_id, Some(20));
        assert!(!plan.discord_permissions_verified);
        std::fs::write(
            &thread,
            r#"{"id":"21","guild_id":"99","parent_id":"20","type":11}"#,
        )
        .unwrap();
        let error = context
            .message_plan("message.list", Some(10), None, Some(21))
            .unwrap_err();
        assert!(error.to_string().contains("metadata"));
        std::fs::write(
            &thread,
            r#"{"id":"21","guild_id":"10","parent_id":"99","type":11}"#,
        )
        .unwrap();
        let error = context
            .message_plan("message.list", Some(10), None, Some(21))
            .unwrap_err();
        assert!(error.to_string().contains("metadata"));
    }
    #[test]
    fn requested_missing_policy_never_falls_back_to_unrestricted() {
        let dir = tempfile::tempdir().unwrap();
        let home = AppHome(dir.path().to_owned());
        let error = ReadContext::load_with(
            ReadOptions {
                read_policy: Some(
                    home.file_path("missing.json")
                        .to_string_lossy()
                        .into_owned(),
                ),
                ..ReadOptions::default()
            },
            &home,
        )
        .unwrap_err();
        assert!(error.to_string().contains("Failed to read"));
    }
    #[test]
    fn absent_default_policy_is_optional_but_existing_policy_is_enforced() {
        let dir = tempfile::tempdir().unwrap();
        let home = AppHome(dir.path().to_owned());
        let context = ReadContext::load_with(ReadOptions::default(), &home).unwrap();
        assert!(context.policy.is_none());
        context.guild_plan("channel.list", Some(30)).unwrap();

        std::fs::write(
            home.file_path("read-policy.json"),
            r#"{"version":1,"guild-ids":[10],"channel-ids":[20],"include-child-threads":false,"allow-user-list":false}"#,
        )
        .unwrap();
        let context = ReadContext::load_with(ReadOptions::default(), &home).unwrap();
        assert!(context.policy.is_some());
        context.guild_plan("channel.list", Some(10)).unwrap();
        let _redacted_error = context.guild_plan("channel.list", Some(30)).unwrap_err();
    }
    #[test]
    fn default_policy_metadata_errors_never_disable_restrictions() {
        let error = load_default_policy(
            Path::new("read-policy.json"),
            Err(std::io::ErrorKind::PermissionDenied.into()),
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Failed to inspect default read policy")
        );
    }
    #[test]
    fn existing_default_policy_with_unreadable_target_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("read-policy.json");
        // Simulate successful link metadata with a missing target, without symlink privileges.
        let metadata = std::fs::symlink_metadata(dir.path());
        let error = load_default_policy(&path, metadata).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Failed to read local read policy")
        );
    }
    #[test]
    fn absent_selection_requires_explicit_context_before_any_credential_access() {
        let dir = tempfile::tempdir().unwrap();
        let home = AppHome(dir.path().to_owned());
        std::fs::create_dir(home.file_path("discord-bot-token.txt")).unwrap();
        // Named profiles may exist while none is active. Never select the first.
        Profiles::update(&home, |profiles| {
            profiles.set(Profile {
                name: "saved".into(),
                guild_id: 10,
                channel_id: Some(20),
                broker: None,
            })
        })
        .unwrap();
        let context = ReadContext::load_with(ReadOptions::default(), &home).unwrap();
        assert!(context.profile.is_none());
        assert_eq!(
            context
                .guild_plan("channel.list", Some(30))
                .unwrap()
                .guild_id,
            Some(30)
        );
        let plan = context
            .message_plan("message.list", Some(30), Some(40), None)
            .unwrap();
        assert_eq!((plan.guild_id, plan.channel_id), (Some(30), Some(40)));
        let error = context.guild_plan("channel.list", None).unwrap_err();
        assert!(error.to_string().contains("--guild-id"));
        let error = context
            .message_plan("message.list", Some(30), None, None)
            .unwrap_err();
        assert!(error.to_string().contains("--channel-id/--thread-id"));
        let error = context
            .message_plan("message.list", None, Some(40), None)
            .unwrap_err();
        assert!(error.to_string().contains("--guild-id"));
    }
}
