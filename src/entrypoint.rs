use crate::cli::log_format::LogFormat;
use crate::failure_record::FailureRecord;
use std::io::Write;

/// Binary boundary. Failures never print an unreviewed error chain or command arguments.
/// Successful builtin requests retain Figue presentation; NDJSON parse failures use a safe record.
#[must_use]
pub fn entrypoint() -> std::process::ExitCode {
    entrypoint_with_failure_message(default_failure_message)
}

/// Allow a consumer to classify failures into reviewed static messages without printing raw errors.
#[must_use]
pub fn entrypoint_with_failure_message(
    classify: fn(&eyre::Report) -> &'static str,
) -> std::process::ExitCode {
    // This narrow scan only selects emergency encoding if normal startup cannot complete.
    let format = requested_format();
    std::panic::set_hook(Box::new(move |_| {
        let _ = report_failure(format, "Process panicked");
    }));
    match crate::main() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            if let Some(outcome) = error.downcast_ref::<crate::cli::watch::WatchCliExitCode>() {
                return std::process::ExitCode::from(outcome.code());
            }
            let message = error
                .downcast_ref::<crate::cli_input_error::CliInputError>()
                .map_or_else(|| classify(&error).to_owned(), ToString::to_string);
            tracing::error!(target: "teamy::failure_file", event_kind = "failure", "{message}");
            let _ = report_failure(format, &message);
            std::process::ExitCode::FAILURE
        }
    }
}

pub(crate) fn requested_format() -> LogFormat {
    let args: Vec<_> = std::env::args_os()
        .skip(1)
        .take_while(|arg| arg != "--")
        .collect();
    if args
        .windows(2)
        .any(|pair| pair[0] == "--log-format" && pair[1] == "ndjson")
        || args.iter().any(|arg| arg == "--log-format=ndjson")
    {
        LogFormat::Ndjson
    } else {
        LogFormat::Text
    }
}

fn report_failure(format: LogFormat, message: &str) -> std::io::Result<()> {
    let mut stderr = std::io::stderr().lock();
    match format {
        LogFormat::Text => writeln!(stderr, "ERROR: {message}"),
        LogFormat::Ndjson => {
            let record = FailureRecord {
                level: "ERROR",
                event_kind: "failure",
                message,
            };
            let encoded = facet_json::to_string(&record).map_err(std::io::Error::other)?;
            writeln!(stderr, "{encoded}")
        }
    }
}

pub(crate) fn finish_parse(
    outcome: figue::DriverOutcome<crate::cli::Cli>,
) -> eyre::Result<crate::cli::Cli> {
    match outcome.into_result() {
        Ok(output) => Ok(output.value),
        Err(error) => {
            // Parse diagnostics can contain the entire argv, including mail bodies or secrets.
            if !error.is_success() {
                return Err(crate::cli_input_error::CliInputError::InvalidValue.into());
            }
            Ok(figue::DriverOutcome::<crate::cli::Cli>::err(error).unwrap())
        }
    }
}

pub(crate) fn default_failure_message(error: &eyre::Report) -> &'static str {
    if let Some(kind) =
        error.downcast_ref::<crate::discord::watch_runtime_error::WatchRuntimeError>()
    {
        return kind.message();
    }
    if let Some(kind) = error.downcast_ref::<crate::watcher::WatcherError>() {
        return kind.message();
    }
    if let Some(kind) = error.downcast_ref::<crate::watch_consumers::ConsumerError>() {
        return kind.message();
    }
    #[cfg(windows)]
    if let Some(kind) = error.downcast_ref::<teamy_broker_sdk::FailureKind>() {
        return kind.message();
    }
    if let Some(kind) = error.downcast_ref::<crate::discord::message_list_error::MessageListError>()
    {
        kind.message()
    } else if let Some(kind) =
        error.downcast_ref::<crate::discord::configuration_error::ConfigurationError>()
    {
        kind.message()
    } else if error.to_string().contains("Re-run with --force") {
        "Destination contains generated files; use --force only for intentional overwrite"
    } else if error.to_string() == "invalid command line" {
        "Invalid command line; use --help"
    } else if error.to_string() == "credential request denied" {
        "Broker denied credential request; approval may have been denied or timed out"
    } else if error.to_string() == "Broker account required" {
        "Configure TEAMY_DISCORD_BROKER_ACCOUNT or --broker-account"
    } else if error.to_string() == "Broker item reference required" {
        "Configure TEAMY_DISCORD_BROKER_SECRET_REF or --broker-secret-ref"
    } else if error.to_string().contains("Conflicting")
        || error.to_string().contains("both selected")
    {
        "Conflicting credential inputs; select one explicit source"
    } else if error.to_string().starts_with("Unknown profile") {
        "Unknown profile; use profile list"
    } else if error.to_string() == "Provide --account and/or --secret-ref for profile broker set" {
        "Provide --account and/or --secret-ref for profile broker set"
    } else if error.to_string() == "Choose --all or --account/--secret-ref for profile broker clear"
    {
        "Choose --all or --account/--secret-ref for profile broker clear"
    } else if error.to_string() == "Select a profile with profile use <name>" {
        "Select a profile with profile use <name>"
    } else if error.to_string()
        == "Explicit bot token required; use --bot-token or TEAMY_DISCORD_BOT_TOKEN"
    {
        "Explicit bot token required; use --bot-token or TEAMY_DISCORD_BOT_TOKEN"
    } else if error.to_string()
        == "Unable to inspect legacy token storage; review app home permissions"
    {
        "Unable to inspect legacy token storage; review app home permissions"
    } else {
        "Command failed; check configuration and diagnostic events"
    }
}

/// Migration diagnostics are visible even when ordinary logs are disabled.
pub(crate) fn report_legacy_token_storage(format: LogFormat) -> std::io::Result<()> {
    let message = "Legacy plaintext token storage detected: discord-bot-token.txt is never used. Review/revoke its credential and arrange explicit cleanup; no contents were read and no file was changed. Use profile broker set/show/clear for references.";
    let mut stderr = std::io::stderr().lock();
    match format {
        LogFormat::Text => writeln!(stderr, "WARN: {message}"),
        LogFormat::Ndjson => {
            let record = FailureRecord {
                level: "WARN",
                event_kind: "migration",
                message,
            };
            let encoded = facet_json::to_string(&record).map_err(std::io::Error::other)?;
            writeln!(stderr, "{encoded}")
        }
    }
}

#[cfg(all(test, windows))]
mod broker_failure_tests {
    #[test]
    fn native_denial_timeout_and_upstream_failure_have_distinct_static_diagnostics() {
        for kind in [
            teamy_broker_sdk::FailureKind::UserDenied,
            teamy_broker_sdk::FailureKind::ApprovalTimedOut,
            teamy_broker_sdk::FailureKind::UpstreamReadFailed,
        ] {
            let report = eyre::Report::new(kind);
            assert_eq!(super::default_failure_message(&report), kind.message());
        }
    }
}
