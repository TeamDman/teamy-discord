use crate::discord::read_context::ReadContext;
use arbitrary::Arbitrary;
use eyre::Context;
use eyre::Result;
use facet::Facet;
use figue::{self as args};
use serenity::all::GuildId;

/// List active threads in a guild.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct ThreadListArgs {
    /// Guild id whose active threads should be listed.
    #[facet(args::named)]
    pub guild_id: Option<u64>,
}

impl ThreadListArgs {
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
        let plan = context.guild_plan("thread.list", self.guild_id)?;
        if context.emit_plan(&plan)? {
            return Ok(());
        }
        let guild_id = GuildId::new(
            plan.guild_id
                .ok_or_else(|| eyre::eyre!("Missing guild in query plan"))?,
        );
        let http = context
            .http(&plan.operation, plan.guild_id, plan.channel_id)
            .await?;
        http.run(context.check_live_guild(&http, guild_id.get()))
            .await?;
        let threads = http
            .run(async {
                http.get_guild_active_threads(guild_id)
                    .await
                    .map_err(eyre::Report::from)
            })
            .await
            .wrap_err_with(|| {
                format!("Failed to list active threads for guild {}", guild_id.get())
            })?;
        let mut threads = threads.threads;
        if let Some(policy) = &context.policy {
            threads.retain(|channel| {
                policy.allows_channel(
                    channel.id.get(),
                    channel.parent_id.map(serenity::all::ChannelId::get),
                    true,
                )
            });
        }
        crate::json_stdout::print_serde_json(&threads)
    }
}
