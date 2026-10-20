pub mod archive;
pub mod attachment;
pub mod cache;
pub mod channel;
pub mod facet_shape;
pub mod global_args;
pub mod guild;
pub mod home;
pub mod invite;
pub mod log_format;
pub mod message;
pub mod output;
pub mod profile;
pub mod thread;
pub mod user;
pub mod watch;

use crate::cli::cache::CacheArgs;
use crate::cli::global_args::GlobalArgs;
use crate::cli::home::HomeArgs;
use crate::cli::invite::InviteArgs;
use arbitrary::Arbitrary;
use eyre::Context;
use facet::Facet;
use figue::FigueBuiltins;
use figue::{self as args};

/// Query Discord using local profiles and archive content to the filesystem.
///
/// Environment variables:
/// - `TEAMY_DISCORD_HOME_DIR` overrides the resolved application home directory.
/// - `TEAMY_DISCORD_CACHE_DIR` overrides the resolved cache directory.
/// - `TEAMY_DISCORD_BOT_TOKEN` supplies the Discord bot token for commands that contact Discord.
/// - `TEAMY_DISCORD_OUTPUT_DIR` overrides the persisted output directory preference.
/// - Legacy noncredential names remain fallback inputs; token aliases conflict if both set.
/// - Live queries default to broker memory; configure `TEAMY_DISCORD_BROKER_ACCOUNT` and `TEAMY_DISCORD_BROKER_SECRET_REF`.
/// - `TEAMY_DISCORD_CREDENTIAL_PROVIDER` selects an explicit provider; no plaintext token fallback.
/// - `RUST_LOG` provides a tracing filter when `--log-filter` is omitted.
// cli[impl parser.args-consistent]
// cli[impl parser.roundtrip]
// tool[impl cli.help.describes-behavior]
// tool[impl cli.help.describes-argv]
// tool[impl cli.help.describes-environment]
#[derive(Facet, Arbitrary, Debug)]
pub struct Cli {
    /// Global arguments (`debug`, `log_filter`, `log_file`).
    #[facet(flatten)]
    pub global_args: GlobalArgs,

    /// Standard CLI options (help, version, completions).
    #[facet(flatten)]
    #[arbitrary(default)]
    // tool[impl cli.help.position-independent]
    pub builtins: FigueBuiltins,

    /// The command to run.
    #[facet(args::subcommand)]
    pub command: Command,
}

impl PartialEq for Cli {
    fn eq(&self, other: &Self) -> bool {
        // Ignore builtins in comparison since FigueBuiltins doesn't implement PartialEq
        self.global_args == other.global_args && self.command == other.command
    }
}

impl Cli {
    /// # Errors
    ///
    /// This function will return an error if the tokio runtime cannot be built or if the command fails.
    pub fn invoke(self) -> eyre::Result<crate::cli::output::CliOutput> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .wrap_err("Failed to build tokio runtime")?;
        runtime.block_on(async move { self.command.invoke().await })
    }
}

/// Discord queries, local profiles and archival operations.
// cli[impl command.surface.core]
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum Command {
    /// Cache-related commands.
    Cache(CacheArgs),
    /// Home-related commands.
    Home(HomeArgs),
    /// Print and optionally open the Discord bot invite URL.
    // cli[impl command.surface.invite]
    Invite(InviteArgs),
    /// Query channels in the selected guild.
    Channel(crate::cli::channel::ChannelArgs),
    /// Query messages in the selected channel or thread.
    Message(crate::cli::message::MessageArgs),
    /// Query active threads in the selected guild.
    Thread(crate::cli::thread::ThreadArgs),
    /// Query guild users.
    User(crate::cli::user::UserArgs),
    /// Query guilds visible to the bot.
    Guild(crate::cli::guild::GuildArgs),
    /// Query attachments on messages.
    Attachment(crate::cli::attachment::AttachmentArgs),
    /// Manage nonsecret local defaults.
    Profile(crate::cli::profile::ProfileArgs),
    /// Archive sync, checkpoints and output location.
    Archive(crate::cli::archive::ArchiveArgs),
    /// Foreground read-only capture daemon and lightweight acknowledged IPC consumers.
    Watch(crate::cli::watch::WatchArgs),
}

impl Command {
    /// # Errors
    ///
    /// This function will return an error if the subcommand fails.
    pub async fn invoke(self) -> eyre::Result<crate::cli::output::CliOutput> {
        match self {
            Command::Cache(args) => args.invoke().await,
            Command::Home(args) => args.invoke().await,
            Command::Invite(args) => args.invoke().await,
            Command::Channel(args) => args.invoke().await,
            Command::Message(args) => args.invoke().await,
            Command::Thread(args) => args.invoke().await,
            Command::User(args) => args.invoke().await,
            Command::Guild(args) => args.invoke().await,
            Command::Attachment(args) => args.invoke().await,
            Command::Profile(args) => args.invoke(),
            Command::Archive(args) => args.invoke().await,
            Command::Watch(args) => args.invoke().await,
        }
    }
}
