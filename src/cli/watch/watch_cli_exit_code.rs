/// A reviewed daemon outcome already rendered to stdout, with a preserved process status.
#[derive(Debug)]
pub struct WatchCliExitCode(u8);

impl WatchCliExitCode {
    #[must_use]
    pub const fn new(code: u8) -> Self {
        Self(code)
    }

    #[must_use]
    pub const fn code(&self) -> u8 {
        self.0
    }
}

impl std::fmt::Display for WatchCliExitCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Watcher operation did not complete successfully; inspect its JSON outcome")
    }
}

impl std::error::Error for WatchCliExitCode {}
