use super::AppHome;
/// Detect a legacy file, directory or symlink by metadata only. Never read or delete it.
/// # Errors
/// Returns a reviewed error if presence cannot be inspected.
pub fn is_present(home: &AppHome) -> eyre::Result<bool> {
    match std::fs::symlink_metadata(home.file_path("discord-bot-token.txt")) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_redacted_error) => {
            eyre::bail!("Unable to inspect legacy token storage; review app home permissions")
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detection_never_requires_reading_the_token() {
        let dir = tempfile::tempdir().unwrap();
        let home = AppHome(dir.path().to_owned());
        assert!(!is_present(&home).unwrap());
        std::fs::create_dir(home.file_path("discord-bot-token.txt")).unwrap();
        assert!(is_present(&home).unwrap());
    }
}
