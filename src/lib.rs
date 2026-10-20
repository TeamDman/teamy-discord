#![deny(clippy::disallowed_methods)]
#![deny(clippy::disallowed_macros)]

pub mod cli_input;
pub mod cli_input_error;
pub mod elapsed_duration;
mod entrypoint;
mod failure_record;
pub use entrypoint::entrypoint;
pub use entrypoint::entrypoint_with_failure_message;
pub mod archive;
pub mod cli;
pub mod discord;
pub mod json_stdout;
pub mod logging_init;
pub mod paths;
pub mod watch_consumers;
pub mod watch_daemon;
pub mod watcher;

use crate::cli::Cli;

/// Version string combining package version and git revision.
// tool[impl cli.version.includes-semver]
// tool[impl cli.version.includes-git-revision]
const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (rev ",
    env!("GIT_REVISION"),
    ")"
);

/// Entrypoint for the program.
///
/// # Errors
///
/// This function will return an error if `color_eyre` installation, CLI parsing, logging initialization, or command execution fails.
///
/// # Panics
///
/// Panics if the CLI schema is invalid (should never happen with correct code).
// repo[impl implementation.present]
pub fn main() -> eyre::Result<()> {
    // Install color_eyre for better error reports
    // Binary entrypoint owns safe diagnostic hooks; library callers own their hooks.

    // Parse command line arguments using figue
    // unwrap() is figue's intended CLI entry behavior:
    // it exits with proper codes for --help/--version/completions/parse-errors.
    let arguments = cli_input::prepare::<Cli>(std::env::args_os().skip(1))?;
    let cli: figue::DriverOutcome<Cli> = figue::Driver::new(
        figue::builder::<Cli>()
            .expect("schema should be valid")
            .cli(move |cli| cli.args(arguments).strict())
            .help(move |help| {
                help.version(VERSION)
                    .include_implementation_source_file(true)
                    .include_implementation_git_url("TeamDman/teamy-discord", env!("GIT_REVISION"))
            })
            .build(),
    )
    .run();
    let cli = entrypoint::finish_parse(cli)?;
    if paths::legacy_token_storage::is_present(&paths::APP_HOME)? {
        entrypoint::report_legacy_token_storage(cli.global_args.log_format.unwrap_or_default())?;
    }

    // Initialize logging
    logging_init::init_logging(&cli.global_args)?;

    #[cfg(windows)]
    {
        // Enable ANSI support on Windows
        // This fails in a pipe scenario, so we ignore the error
        let _ = teamy_windows::console::enable_ansi_support();

        // Warn if UTF-8 is not enabled on Windows
        #[cfg(windows)]
        teamy_windows::string::warn_if_utf8_not_enabled();
    };

    let command_name = match &cli.command {
        cli::Command::Cache(_) => "cache",
        cli::Command::Home(_) => "home",
        cli::Command::Invite(_) => "invite",
        cli::Command::Channel(_) => "channel",
        cli::Command::Message(_) => "message",
        cli::Command::Thread(_) => "thread",
        cli::Command::User(_) => "user",
        cli::Command::Guild(_) => "guild",
        cli::Command::Attachment(_) => "attachment",
        cli::Command::Profile(_) => "profile",
        cli::Command::Archive(_) => "archive",
        cli::Command::Watch(_) => "watch",
    };
    tracing::debug!(command = command_name, "Dispatching command");
    let result = (|| -> eyre::Result<()> {
        let _command_span = tracing::info_span!("command", command = command_name).entered();
        let started = std::time::Instant::now();
        let output = cli.invoke()?;
        elapsed_duration::warn_if_exceeded(
            command_name,
            started.elapsed(),
            output.elapsed_duration_warn_threshold,
        );
        Ok(())
    })();
    tracing::debug!(
        command = command_name,
        success = result.is_ok(),
        "Command finished"
    );
    result
}
