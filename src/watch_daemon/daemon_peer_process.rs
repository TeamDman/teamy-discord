#[cfg(windows)]
use std::os::windows::io::AsRawHandle;
#[cfg(windows)]
use std::os::windows::io::OwnedHandle;
/// Retains the verified server process object so PID reuse cannot misclassify exit.
#[cfg(windows)]
#[derive(Debug)]
pub(super) struct DaemonPeerProcess {
    pub handle: OwnedHandle,
}
#[cfg(windows)]
impl DaemonPeerProcess {
    pub async fn exited_for_approval(&self) -> bool {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(250);
        loop {
            let mut code = 0;
            // SAFETY: this owned query handle remains live for the call.
            let queried = unsafe {
                windows_sys::Win32::System::Threading::GetExitCodeProcess(
                    self.handle.as_raw_handle().cast(),
                    &raw mut code,
                )
            };
            if queried == 0 {
                return false;
            }
            if code != 259 {
                return code == 3;
            }
            if tokio::time::Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }
}
