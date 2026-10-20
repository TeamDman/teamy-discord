use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;

/// Show the home path.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct HomeShowArgs;

impl HomeShowArgs {
    /// # Errors
    ///
    /// This function does not return any errors.
    pub async fn invoke(self) -> Result<crate::cli::output::CliOutput> {
        let threshold = Some(std::time::Duration::from_secs(1));
        self.invoke_work().await?;
        Ok(crate::cli::output::CliOutput::none(threshold))
    }

    #[expect(clippy::unused_async)]
    async fn invoke_work(self) -> Result<()> {
        println!("{}", crate::paths::APP_HOME.display());
        Ok(())
    }
}
