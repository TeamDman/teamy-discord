use crate::discord::read_context::ReadContext;
use arbitrary::Arbitrary;
use eyre::Context;
use eyre::Result;
use facet::Facet;
use figue::{self as args};
use serenity::all::GuildId;

/// List users in a guild.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct UserListArgs {
    /// Guild id whose users should be listed.
    #[facet(args::named)]
    pub guild_id: Option<u64>,

    /// Start listing after this user id.
    #[facet(args::named)]
    pub after_user_id: Option<u64>,

    /// Maximum number of users to list.
    #[facet(args::named)]
    pub limit: Option<u64>,
}

impl UserListArgs {
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
        eyre::ensure!(
            self.after_user_id != Some(0),
            "--after-user-id must be nonzero"
        );
        let plan = context.guild_plan("user.list", self.guild_id)?;
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
        let users = http
            .run(async {
                http.get_guild_members(
                    guild_id,
                    Some(crate::discord::live::normalize_user_limit(self.limit)),
                    self.after_user_id,
                )
                .await
                .map_err(eyre::Report::from)
            })
            .await
            .wrap_err_with(|| format!("Failed to list users for guild {}", guild_id.get()))?;
        crate::json_stdout::print_serde_json(&users)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discord::read_options::ReadOptions;

    #[tokio::test]
    async fn zero_user_cursor_is_rejected_before_planning_or_credentials() {
        for plan in [false, true] {
            let context = ReadContext {
                options: ReadOptions {
                    plan,
                    ..ReadOptions::default()
                },
                profile: None,
                policy: None,
                archive_root: None,
            };
            let args = UserListArgs {
                guild_id: Some(10),
                after_user_id: Some(0),
                limit: Some(1),
            };
            let error = args.invoke(&context).await.unwrap_err();
            assert_eq!(error.to_string(), "--after-user-id must be nonzero");
        }
    }
}
