# Logging and progress

Command results use stdout. Diagnostics use stderr and, when explicitly selected,
an additional log file. The CLI has no `--output-format` option.

| Setting | Default |
| --- | --- |
| `--log-format text\|ndjson` | text |
| `--log-filter DIRECTIVES` | explicit flag, then `--debug`, then `RUST_LOG`, then info |
| `--log-file FILE` | no file sink |
| `--log-file-format text\|ndjson` | ndjson |
| `--log-file-filter DIRECTIVES` | debug, independent of stderr and environment |

An explicit stderr filter wins over `--debug`. Use `--log-filter`; this consumer
does not expose a `--log-level` alias. Filters accept EnvFilter directive lists
with regex field matching disabled. File and stderr filters apply independently,
so an INFO stderr filter does not suppress DEBUG file events. Tracy uses a
separate collection filter.

Log files append and require a filename. Missing parent directories are created.
A mutex covers each complete formatted file record. File output and NDJSON
stderr contain no ANSI escapes. Filters are validated before file effects;
subscriber setup failures are errors. Logs have no fsync or rotation guarantee.

The binary emits reviewed failure records without raw error chains, arguments,
paths, or panic payloads. In NDJSON mode these remain JSON lines even if startup
fails or ordinary diagnostics are disabled. Help, version, and completions retain
their generated presentation. Library callers of `main()` own their error and
panic presentation.

Redaction depends on each producer's fields and formatting. Do not log
credentials, message bodies, raw arguments, or arbitrary error/panic payloads.
Sensitive instrumentation uses `skip_all` and reviewed fields. Synthetic tests
check that startup failures do not disclose a supplied marker.

Dispatch and completion events use DEBUG at their call sites. Operations can
report counts, bytes, phases, rates, or ETA directly through tracing; there is no
mandatory progress schema. Command spans retain timing information.

## Command completion budgets

Each successful command returns `CliOutput.elapsed_duration_warn_threshold`.
The receipt carries execution metadata after the command emits its result;
it does not change stdout data. Local inspection/configuration receives one
second; detached directory opening receives three seconds. Invite lookup receives
30 seconds, plus three when opening a browser. Read-query plans receive one
second. Broker queries, archive sync, and watcher operations explicitly use
`None` where approval, network, or waiting time is unbounded.

The top level measures invocation including command output, then warns only when
elapsed time exceeds the returned budget. Failure returns no receipt and no
budget comparison. This is a completion-time warning, not a live watchdog.
See [completion budget details](command-completion-budgets.md).
