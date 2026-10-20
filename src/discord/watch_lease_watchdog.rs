use super::leased_http::LeasedHttp;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread::JoinHandle;
use std::time::Duration;

/// Process-bound safety net for Serenity tasks retaining credential-bearing HTTP copies.
///
/// This guard is deliberately used only by the explicit foreground daemon command.
/// Expiry terminates that process with status 3, including stalled async runtimes.
/// Persisted captures, claims and acknowledgements survive; no lease is renewed.
#[derive(Debug)]
pub struct WatchLeaseWatchdog {
    stopped: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl WatchLeaseWatchdog {
    /// # Errors
    /// Requires an original valid broker lease and a dedicated supervisor thread.
    pub fn arm(http: Arc<LeasedHttp>) -> eyre::Result<Self> {
        eyre::ensure!(
            http.expires_unix_ms().is_some(),
            "Watcher daemon requires a bounded broker lease"
        );
        http.check().map_err(|_redacted_error| {
            eyre::eyre!(super::watch_runtime_error::WatchRuntimeError::ApprovalRequired)
        })?;
        Self::start(move || http.check().is_ok(), || std::process::exit(3))
    }

    /// Keep the original cutoff armed across Gateway construction/cleanup errors.
    /// One bounded supervisor is intentionally retained by the foreground daemon
    /// process. CLI process exit releases it; no new lease is acquired in-process.
    pub fn retain_until_process_exit(self) {
        std::mem::forget(self);
    }

    fn start(
        valid: impl Fn() -> bool + Send + 'static,
        expired: impl FnOnce() + Send + 'static,
    ) -> eyre::Result<Self> {
        let stopped = Arc::new(AtomicBool::new(false));
        let observed = Arc::clone(&stopped);
        let worker = std::thread::Builder::new()
            .name("watch-lease-cutoff".into())
            .spawn(move || {
                while !observed.load(Ordering::Acquire) {
                    if !valid() {
                        expired();
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            })
            .map_err(|_redacted_error| eyre::eyre!("Unable to start watcher lease supervisor"))?;
        Ok(Self {
            stopped,
            worker: Some(worker),
        })
    }
}

impl Drop for WatchLeaseWatchdog {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _finished = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::process::Stdio;
    use std::time::Instant;

    #[test]
    fn dropping_guard_disarms_the_supervisor() {
        let expired = Arc::new(AtomicBool::new(false));
        let valid = Arc::new(AtomicBool::new(true));
        let checked = Arc::clone(&valid);
        let recorded = Arc::clone(&expired);
        let guard = WatchLeaseWatchdog::start(
            move || checked.load(Ordering::Acquire),
            move || recorded.store(true, Ordering::Release),
        )
        .unwrap();
        drop(guard);
        valid.store(false, Ordering::Release);
        std::thread::sleep(Duration::from_millis(30));
        assert!(!expired.load(Ordering::Acquire));
    }

    #[test]
    fn expiry_terminates_a_process_even_with_a_stalled_main_thread() {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "discord::watch_lease_watchdog::tests::expiry_process_fixture",
                "--ignored",
            ])
            .env("TEAMY_WATCHER_CUTOFF_FIXTURE", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000)
        };
        let mut child = command.spawn().unwrap();
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert_eq!(status.code(), Some(3));
                break;
            }
            if Instant::now() >= end {
                let _killed = child.kill();
                let _waited = child.wait();
                panic!("Watcher lease supervisor did not terminate the fixture process");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    #[ignore = "subprocess fixture invoked by expiry_terminates_a_process_even_with_a_stalled_main_thread"]
    fn expiry_process_fixture() {
        assert_eq!(
            std::env::var("TEAMY_WATCHER_CUTOFF_FIXTURE").as_deref(),
            Ok("1")
        );
        let deadline = Instant::now() + Duration::from_millis(75);
        let _guard =
            WatchLeaseWatchdog::start(move || Instant::now() < deadline, || std::process::exit(3))
                .unwrap();
        loop {
            std::thread::park();
        }
    }
}
