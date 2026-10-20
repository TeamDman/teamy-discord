use super::SyncCheckpointArgs;
use super::SyncCheckpointCommand;
use super::SyncCheckpointRestoreArgs;
use super::SyncMembersArgs;
use super::SyncMessagesArgs;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
use figue::{self as args};
use std::path::PathBuf;

/// Synchronize Discord content into the configured output directory.
#[derive(Facet, Arbitrary, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct SyncArgs {
    /// Discord bot token. If omitted, requires one explicit token environment variable; never saves a token.
    #[facet(args::named)]
    pub bot_token: Option<String>,

    /// Override the output directory for this run.
    #[facet(args::named)]
    pub output_dir: Option<String>,

    /// Optional sync subcommand.
    #[facet(args::subcommand)]
    pub command: Option<SyncCommand>,
}

/// Nested sync commands.
// cli[impl command.surface.sync]
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum SyncCommand {
    /// Synchronize archived messages and attachments only.
    Messages(SyncMessagesArgs),
    /// Synchronize archived guild-scoped member records and avatars from archived messages.
    Members(SyncMembersArgs),
    /// Checkpoint maintenance commands.
    // cli[impl command.surface.sync-checkpoint]
    Checkpoint(SyncCheckpointArgs),
}

// cli[impl command.surface.sync-checkpoint]

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedSync {
    pub output_dir: crate::paths::ResolvedOutputDir,
    pub state: crate::paths::SyncStateLayout,
}

// cli[impl sync.checkpoint.restore.dry-run]
pub(super) fn restore_sync_checkpoint(
    prepared: &PreparedSync,
    dry_run: bool,
) -> Result<crate::archive::SyncCheckpointRestoreSummary> {
    crate::archive::restore_checkpoint_from_output(
        prepared.output_dir.path.as_path(),
        &prepared.state,
        dry_run,
    )
}

/// # Errors
///
/// This function will return an error if no output directory can be resolved,
/// if the target directory cannot be created, or if the cache-backed sync state
/// layout cannot be prepared.
// cli[impl sync.requires-output-dir]
pub fn prepare_sync(
    app_home: &crate::paths::AppHome,
    cache_home: &crate::paths::CacheHome,
    command_line_output_dir: Option<PathBuf>,
    environment_output_dir: Option<&str>,
) -> Result<PreparedSync> {
    let output_dir = crate::paths::require_output_dir_with(
        app_home,
        command_line_output_dir,
        environment_output_dir,
    )?;
    output_dir.ensure_dir()?;
    let state = crate::paths::ensure_sync_state_layout(cache_home, output_dir.path.as_path())?;
    Ok(PreparedSync { output_dir, state })
}

/// Resolve archive/checkpoint locations without creating any directories.
/// # Errors
/// Returns an error when no valid output directory is configured.
pub fn prepare_sync_read_only(
    app_home: &crate::paths::AppHome,
    cache_home: &crate::paths::CacheHome,
    command_line_output_dir: Option<PathBuf>,
    environment_output_dir: Option<&str>,
) -> Result<PreparedSync> {
    let output_dir = crate::paths::require_output_dir_with(
        app_home,
        command_line_output_dir,
        environment_output_dir,
    )?;
    let state = crate::paths::sync_state_layout(cache_home, output_dir.path.as_path());
    Ok(PreparedSync { output_dir, state })
}
impl SyncArgs {
    /// # Errors
    ///
    /// This function will return an error if no output directory or bot token can be resolved,
    /// if archive directories cannot be created, or if the Discord sync fails.
    // cli[impl sync.requires-token]
    pub async fn invoke(self) -> Result<crate::cli::output::CliOutput> {
        let threshold = None;
        self.invoke_work().await?;
        Ok(crate::cli::output::CliOutput::none(threshold))
    }

    async fn invoke_work(self) -> Result<()> {
        let environment_output_dir = crate::paths::environment_preference(
            crate::paths::OUTPUT_DIR_ENV_VAR,
            crate::paths::LEGACY_OUTPUT_DIR_ENV_VAR,
        );
        let dry_run = matches!(
            &self.command,
            Some(SyncCommand::Checkpoint(SyncCheckpointArgs {
                command: SyncCheckpointCommand::Restore(SyncCheckpointRestoreArgs {
                    dry_run: true
                }),
            }))
        );
        let prepare = if dry_run {
            prepare_sync_read_only
        } else {
            prepare_sync
        };
        let prepared = prepare(
            &crate::paths::APP_HOME,
            &crate::paths::CACHE_DIR,
            self.output_dir.map(PathBuf::from),
            environment_output_dir.as_deref(),
        )?;

        match self.command {
            None => {
                let resolved_token = crate::paths::resolve_bot_token(self.bot_token.as_deref())?;
                let mut summary = crate::archive::run_sync(
                    prepared.output_dir.path.as_path(),
                    &prepared.state,
                    &resolved_token.token,
                )
                .await?;
                summary.output_dir = prepared.output_dir.path.display().to_string();
                summary.checkpoint_path = prepared.state.checkpoint_path.display().to_string();
                crate::json_stdout::print_facet_json(&summary)?;
            }
            Some(command) => match command {
                SyncCommand::Messages(args) => {
                    let resolved_token =
                        crate::paths::resolve_bot_token(self.bot_token.as_deref())?;
                    let mut summary = args.invoke(&prepared, &resolved_token.token).await?;
                    summary.output_dir = prepared.output_dir.path.display().to_string();
                    summary.checkpoint_path = prepared.state.checkpoint_path.display().to_string();
                    crate::json_stdout::print_facet_json(&summary)?;
                }
                SyncCommand::Members(args) => {
                    let resolved_token =
                        crate::paths::resolve_bot_token(self.bot_token.as_deref())?;
                    let mut summary = args.invoke(&prepared, &resolved_token.token).await?;
                    summary.output_dir = prepared.output_dir.path.display().to_string();
                    summary.checkpoint_path = prepared.state.checkpoint_path.display().to_string();
                    crate::json_stdout::print_facet_json(&summary)?;
                }
                SyncCommand::Checkpoint(args) => {
                    let summary = args.invoke(&prepared)?;
                    crate::json_stdout::print_facet_json(&summary)?;
                }
            },
        }
        Ok(())
    }
}

impl std::fmt::Debug for SyncArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncArgs")
            .field("bot_token", &self.bot_token.as_ref().map(|_| "[REDACTED]"))
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::prepare_sync;
    use super::restore_sync_checkpoint;
    use crate::paths::AppHome;
    use crate::paths::CacheHome;
    use crate::paths::OutputDirSource;
    use crate::paths::resolve_bot_token_from_sources;
    use crate::paths::save_output_dir_preference;
    use tempfile::tempdir;

    #[test]
    fn dry_run_preparation_does_not_create_output_or_checkpoint_directories() {
        let dir = tempdir().unwrap();
        let home = AppHome(dir.path().join("home"));
        let cache = CacheHome(dir.path().join("cache"));
        let output = dir.path().join("archive");
        let prepared =
            super::prepare_sync_read_only(&home, &cache, Some(output.clone()), None).unwrap();
        assert_eq!(prepared.output_dir.path, output);
        assert!(!home.0.exists());
        assert!(!cache.0.exists());
        assert!(!output.exists());
    }
    #[test]
    // cli[verify sync.requires-output-dir]
    fn prepare_sync_uses_explicit_inputs_and_creates_state_dirs() {
        let temp_dir = tempdir().expect("tempdir should be created");
        let app_home = AppHome(temp_dir.path().join("home"));
        let cache_home = CacheHome(temp_dir.path().join("cache"));
        let output_root = temp_dir.path().join("output");

        let prepared = prepare_sync(&app_home, &cache_home, Some(output_root.clone()), None)
            .expect("sync preparation should succeed");

        assert_eq!(prepared.output_dir.path, output_root);
        assert_eq!(prepared.output_dir.source, OutputDirSource::CommandLine);
        assert!(prepared.output_dir.path.exists());
        assert!(prepared.state.target_dir.exists());
        assert_eq!(
            prepared.state.checkpoint_path,
            prepared.state.target_dir.join("checkpoint.json")
        );
    }

    #[test]
    // cli[verify sync.requires-output-dir]
    fn prepare_sync_uses_saved_preference_without_globals() {
        let temp_dir = tempdir().expect("tempdir should be created");
        let app_home = AppHome(temp_dir.path().join("home"));
        let cache_home = CacheHome(temp_dir.path().join("cache"));
        let saved_output_root = temp_dir.path().join("saved-output");
        save_output_dir_preference(&app_home, &saved_output_root)
            .expect("output directory preference should save");

        let prepared = prepare_sync(&app_home, &cache_home, None, None)
            .expect("sync preparation should resolve saved output dir");

        assert_eq!(prepared.output_dir.path, saved_output_root);
        assert_eq!(prepared.output_dir.source, OutputDirSource::Preference);
    }

    #[test]
    // cli[verify sync.requires-output-dir]
    fn prepare_sync_fails_without_any_output_dir_source() {
        let temp_dir = tempdir().expect("tempdir should be created");
        let app_home = AppHome(temp_dir.path().join("home"));
        let cache_home = CacheHome(temp_dir.path().join("cache"));

        let error = prepare_sync(&app_home, &cache_home, None, None)
            .expect_err("sync preparation should fail without an output dir");

        assert!(error.to_string().contains("No output directory configured"));
    }

    #[test]
    // cli[verify sync.requires-token]
    fn sync_token_resolution_fails_without_any_token_source() {
        let error = resolve_bot_token_from_sources(None, None, None)
            .expect_err("sync token resolution should fail without a token");

        assert!(error.to_string().contains("Explicit bot token required"));
    }

    #[test]
    // cli[verify sync.checkpoint.restore.dry-run]
    fn restore_sync_checkpoint_dry_run_compares_without_writing() {
        let temp_dir = tempdir().expect("tempdir should be created");
        let app_home = AppHome(temp_dir.path().join("home"));
        let cache_home = CacheHome(temp_dir.path().join("cache"));
        let output_root = temp_dir.path().join("output");
        let message_dir = output_root
            .join("guilds")
            .join("99")
            .join("channels")
            .join("10")
            .join("messages");
        std::fs::create_dir_all(&message_dir).expect("message dir should exist");
        std::fs::write(
            output_root
                .join("guilds")
                .join("99")
                .join("channels")
                .join("10")
                .join("channel.json"),
            "{}",
        )
        .expect("channel metadata should write");
        std::fs::write(message_dir.join("50.json"), "hello").expect("message should write");

        let prepared = prepare_sync(&app_home, &cache_home, Some(output_root), None)
            .expect("sync preparation should succeed");
        let existing = crate::archive::SyncCheckpoint {
            version: 1,
            targets: vec![crate::archive::SyncTargetCheckpoint {
                guild_id: 99,
                channel_id: 10,
                parent_channel_id: None,
                newest_message_id: Some(999),
                oldest_message_id: Some(999),
                historical_complete: true,
                archived_message_count: Some(999),
                archived_byte_count: Some(999),
            }],
        };
        std::fs::write(
            &prepared.state.checkpoint_path,
            facet_json::to_string_pretty(&existing).expect("checkpoint should serialize"),
        )
        .expect("checkpoint should write");

        let summary =
            restore_sync_checkpoint(&prepared, true).expect("dry-run restore should succeed");

        assert!(summary.dry_run);
        assert!(summary.existing_checkpoint_found);
        assert_eq!(summary.restored_target_count, 1);
        assert_eq!(summary.restored_message_count, 1);
        assert!(summary.comparison.is_some());

        let on_disk: crate::archive::SyncCheckpoint = facet_json::from_str(
            &std::fs::read_to_string(&prepared.state.checkpoint_path)
                .expect("checkpoint should still exist"),
        )
        .expect("checkpoint should parse");
        assert_eq!(on_disk, existing);
    }
}
