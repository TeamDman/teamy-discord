use crate::discord::read_context::ReadContext;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;

/// List guilds visible to the bot.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct GuildListArgs;

impl GuildListArgs {
    /// # Errors
    ///
    /// This function will return an error if the Discord API call fails.
    pub async fn invoke(self, context: &ReadContext) -> Result<crate::cli::output::CliOutput> {
        let threshold = context
            .options
            .plan
            .then_some(std::time::Duration::from_secs(1));
        self.invoke_work(context).await?;
        Ok(crate::cli::output::CliOutput::none(threshold))
    }

    async fn invoke_work(self, context: &ReadContext) -> Result<()> {
        if context.options.plan {
            return crate::json_stdout::print_facet_json(
                &context.policy.as_ref().map(|p| &p.guild_ids),
            );
        }
        let http = context.http("guild.list", None, None).await?;
        let mut guilds = http.run(crate::discord::live::list_guilds(&http)).await?;
        if let Some(policy) = &context.policy {
            guilds.retain(|guild| policy.guild_ids.contains(&guild.id.get()));
        }
        crate::json_stdout::print_serde_json(&guilds)
    }
}
