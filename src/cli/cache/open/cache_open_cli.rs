use arbitrary::Arbitrary;
use eyre::Context;
use eyre::Result;
use facet::Facet;

/// Open the cache path in the platform file manager.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct CacheOpenArgs;

impl CacheOpenArgs {
    /// # Errors
    ///
    /// This function will return an error if the cache directory cannot be created
    /// or the file manager cannot be launched.
    pub async fn invoke(self) -> Result<crate::cli::output::CliOutput> {
        let threshold = Some(std::time::Duration::from_secs(3));
        self.invoke_work().await?;
        Ok(crate::cli::output::CliOutput::none(threshold))
    }

    #[expect(clippy::unused_async)]
    async fn invoke_work(self) -> Result<()> {
        std::fs::create_dir_all(crate::paths::CACHE_DIR.0.as_path())?;
        open::that_detached(crate::paths::CACHE_DIR.0.as_path()).wrap_err_with(|| {
            format!(
                "Failed to open {} in file manager",
                crate::paths::CACHE_DIR.0.display()
            )
        })?;
        Ok(())
    }
}
