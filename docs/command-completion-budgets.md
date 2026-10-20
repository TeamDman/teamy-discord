# Command completion budgets

Each successful command returns `CliOutput` with an explicit
`elapsed_duration_warn_threshold: Option<Duration>`. The constructor is
`CliOutput::none(threshold)`. Commands write their stdout data before returning
this receipt; the budget is not part of the result JSON or text. Parent command
groups forward the leaf receipt.

The top level starts an `Instant` immediately before invocation. After a
successful return it compares elapsed time to the receipt's budget. A WARN event
is emitted only when elapsed time is strictly greater than the threshold, with
`elapsed_duration_ms` and `elapsed_duration_warn_threshold_ms` fields. `None`
explicitly disables this comparison.

If invocation or output fails, `cli.invoke()?` returns before the comparison;
there is no receipt or budget warning for that failure. The ordinary failure
diagnostic still applies. A completion budget cannot detect a hung command or
warn before an operation returns.

Choose thresholds from the operation's expected work. Path/configuration
inspection receives one second and detached directory opening three seconds.
Invite lookup receives 30 seconds, or 33 when it also opens a browser. Query plans
receive one second. Approval-dependent queries, archive sync, and watcher
operations can wait without a fixed upper bound and use explicit exemptions.

Operations may emit progress fields such as counts, bytes, rates, or ETA through
tracing while they run. These events are separate from completion warnings.

When adding a command, choose and return its budget explicitly, preserve its
stdout format, and forward the receipt through dispatch groups. The unit test in
[`elapsed_duration.rs`](../src/elapsed_duration.rs) checks below/equal/above
thresholds and explicit exemption without sleeping.
