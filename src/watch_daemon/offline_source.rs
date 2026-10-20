use super::DaemonCaptureSource;
use crate::watcher::WatcherMessage;
use crate::watcher::WatcherMessageId;
use crate::watcher::WatcherSource;
#[derive(Debug)]
pub(super) struct OfflineSource;
impl WatcherSource for OfflineSource {
    async fn page(
        &mut self,
        _before: Option<WatcherMessageId>,
        _limit: u8,
    ) -> eyre::Result<Vec<WatcherMessage>> {
        eyre::bail!("Offline daemon cannot read Discord")
    }
    async fn get(&mut self, _id: WatcherMessageId) -> eyre::Result<WatcherMessage> {
        eyre::bail!("Offline daemon cannot read Discord")
    }
}
impl DaemonCaptureSource for OfflineSource {
    fn check_lease(&self) -> eyre::Result<()> {
        eyre::bail!("Offline daemon has no credential lease")
    }
}
