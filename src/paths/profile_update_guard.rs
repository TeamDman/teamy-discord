/// Owns both the lock path and staging handle. Never removes another caller's guard.
#[derive(Debug)]
pub(crate) struct ProfileUpdateGuard {
    pub(crate) pending: std::path::PathBuf,
    pub(crate) file: Option<std::fs::File>,
    pub(crate) committed: bool,
}
impl Drop for ProfileUpdateGuard {
    fn drop(&mut self) {
        drop(self.file.take());
        if !self.committed {
            let _ = std::fs::remove_file(&self.pending);
        }
    }
}
