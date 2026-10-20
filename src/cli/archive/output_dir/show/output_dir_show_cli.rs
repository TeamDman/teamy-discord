use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;

/// Show the effective output directory.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct OutputDirShowArgs;

impl OutputDirShowArgs {
    /// # Errors
    ///
    /// This function will return an error if no output directory has been configured.
    // cli[impl path.output-dir.show-resolves-effective]
    pub async fn invoke(self) -> Result<crate::cli::output::CliOutput> {
        let threshold = Some(std::time::Duration::from_secs(1));
        self.invoke_work().await?;
        Ok(crate::cli::output::CliOutput::none(threshold))
    }

    #[expect(clippy::unused_async)]
    async fn invoke_work(self) -> Result<()> {
        let resolved = crate::paths::require_output_dir(None)?;
        println!("{}", resolved.path.display());
        Ok(())
    }
}
