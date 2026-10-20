use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
/// Cancelling a library serve future must not detach its listener or client tasks.
#[derive(Debug)]
pub(super) struct DaemonTransportTask {
    pub task: tokio::task::JoinHandle<eyre::Result<()>>,
    pub stopped: Arc<AtomicBool>,
}
impl Drop for DaemonTransportTask {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        self.task.abort();
    }
}
