use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use teamy_discord::discord::watch_gateway::guarded_capture;
use teamy_discord::discord::watch_runtime_error::WatchRuntimeError;

#[tokio::test]
async fn cancellation_drops_an_inflight_capture_without_a_result() {
    struct Dropped<'a>(&'a AtomicBool);
    impl Drop for Dropped<'_> {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }
    let cancelled = AtomicBool::new(false);
    let dropped = AtomicBool::new(false);
    let capture = guarded_capture(
        async {
            let _guard = Dropped(&dropped);
            std::future::pending::<eyre::Result<()>>().await
        },
        &cancelled,
        || Ok(()),
    );
    let cancel = async {
        tokio::task::yield_now().await;
        cancelled.store(true, Ordering::Release);
    };
    let (result, ()) = tokio::time::timeout(Duration::from_secs(1), async {
        tokio::join!(capture, cancel)
    })
    .await
    .unwrap();
    assert!(matches!(
        result.unwrap_err().downcast_ref::<WatchRuntimeError>(),
        Some(WatchRuntimeError::Cancelled)
    ));
    assert!(dropped.load(Ordering::Acquire));
}

#[tokio::test]
async fn expired_lease_rejects_a_read_that_finished_at_the_boundary() {
    let cancelled = AtomicBool::new(false);
    let expired = AtomicBool::new(false);
    let result = guarded_capture(
        async {
            expired.store(true, Ordering::Release);
            Ok("must not be published")
        },
        &cancelled,
        || {
            eyre::ensure!(
                !expired.load(Ordering::Acquire),
                WatchRuntimeError::ApprovalRequired
            );
            Ok(())
        },
    )
    .await;
    assert!(matches!(
        result.unwrap_err().downcast_ref::<WatchRuntimeError>(),
        Some(WatchRuntimeError::ApprovalRequired)
    ));
}

#[tokio::test]
async fn invalid_initial_lease_never_polls_the_capture_operation() {
    let cancelled = AtomicBool::new(false);
    let invoked = AtomicBool::new(false);
    let result = guarded_capture(
        async {
            invoked.store(true, Ordering::Release);
            Ok(())
        },
        &cancelled,
        || Err(WatchRuntimeError::ApprovalRequired.into()),
    )
    .await;
    assert!(result.is_err());
    assert!(!invoked.load(Ordering::Acquire));
}
