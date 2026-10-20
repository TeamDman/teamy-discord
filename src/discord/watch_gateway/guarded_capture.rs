use crate::discord::watch_runtime_error::WatchRuntimeError;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

/// Cancel an in-flight read when cancellation or the original lease check fails.
/// A completed read is checked again before its result can become capture input.
/// This helper does not stop independently spawned Gateway tasks; their owner
/// must shut down shards and enforce its whole-process hard lease watchdog.
/// # Errors
/// Preserves the lease-check error, cancellation, or the read operation's error.
pub async fn guarded_capture<T>(
    operation: impl Future<Output = eyre::Result<T>>,
    cancelled: &AtomicBool,
    check_lease: impl Fn() -> eyre::Result<()>,
) -> eyre::Result<T> {
    let check = || {
        eyre::ensure!(
            !cancelled.load(Ordering::Acquire),
            WatchRuntimeError::Cancelled
        );
        check_lease()
    };
    check()?;
    tokio::pin!(operation);
    let mut tick = tokio::time::interval(Duration::from_millis(10));
    loop {
        tokio::select! {
            biased;
            _ = tick.tick() => check()?,
            result = &mut operation => {
                check()?;
                return result;
            }
        }
    }
}
