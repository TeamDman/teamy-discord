use sha2::Digest;
use sha2::Sha256;
use std::path::Path;
/// Derived in memory from the absolute state directory and owning Windows SID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DaemonEndpoint(String);
impl DaemonEndpoint {
    /// # Errors
    /// Requires an absolute path and a resolvable current-user identity.
    /// Does not create, open or mutate state files.
    pub fn for_state_dir(path: &Path) -> eyre::Result<Self> {
        eyre::ensure!(
            path.is_absolute(),
            "Watcher state directory must be absolute"
        );
        let absolute = std::path::absolute(path)?;
        #[cfg(windows)]
        let owner = super::pipe_security::current_sid()?;
        #[cfg(not(windows))]
        let owner = "unsupported-platform".to_owned();
        let normalized = absolute.to_string_lossy().replace('/', "\\").to_lowercase();
        let digest = format!(
            "{:x}",
            Sha256::digest(format!("{owner}\n{normalized}").as_bytes())
        );
        Ok(Self(format!(
            r"\\.\pipe\teamy-discord-watch-v1-{}",
            &digest[..32]
        )))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
