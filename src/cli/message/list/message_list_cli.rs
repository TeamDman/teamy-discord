use crate::discord::message_list_request::MessageListRequest;
use crate::discord::message_page_source::MessagePageSource;
use crate::discord::read_context::ReadContext;
use arbitrary::Arbitrary;
use eyre::Context;
use eyre::Result;
use facet::Facet;
use figue::{self as args};
use std::fmt::Write;

/// List messages in a channel or thread.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct MessageListArgs {
    /// Guild ID; overrides the selected profile default.
    #[facet(args::named)]
    pub guild_id: Option<u64>,

    /// Channel id whose messages should be listed.
    #[facet(args::named)]
    pub channel_id: Option<u64>,

    /// Thread id whose messages should be listed.
    #[facet(args::named)]
    pub thread_id: Option<u64>,

    /// Return messages before this RFC3339 timestamp.
    #[facet(args::named)]
    pub before: Option<String>,

    /// Maximum number of messages to list.
    #[facet(args::named)]
    pub limit: Option<u8>,
    /// Filter explicit user mentions: me means the authenticated bot, or use a numeric user ID.
    #[facet(args::named)]
    pub mentions: Option<String>,
    /// Read at most this many recent messages for mentions (default 100, maximum 1000).
    #[facet(args::named)]
    pub scan_limit: Option<u16>,
}

impl MessageListArgs {
    /// # Errors
    ///
    /// This function will return an error if argument resolution fails or the Discord API call fails.
    // cli[impl live.message.before-flag]
    // cli[impl live.message.target-selection]
    pub async fn invoke(self, context: &ReadContext) -> Result<crate::cli::output::CliOutput> {
        let threshold = context
            .options
            .plan
            .then_some(std::time::Duration::from_secs(1));
        self.invoke_work(context).await?;
        Ok(crate::cli::output::CliOutput::none(threshold))
    }

    async fn invoke_work(self, context: &ReadContext) -> Result<()> {
        let plan = context.message_plan(
            "message.list",
            self.guild_id,
            self.channel_id,
            self.thread_id,
        )?;
        let before = crate::discord::live::parse_before_datetime(self.before.as_deref())?;
        let before = before
            .map(crate::discord::live::before_datetime_to_message_id)
            .transpose()?
            .map(serenity::all::MessageId::get);
        let request = MessageListRequest::new(
            plan.channel_id
                .ok_or_else(|| eyre::eyre!("Missing channel in query plan"))?,
            self.limit,
            before,
            self.mentions.as_deref(),
            self.scan_limit,
        )?;
        if context.options.plan {
            if let Some(mentions) = request.mentions {
                crate::json_stdout::print_facet_json(
                    &crate::discord::message_list_plan::MessageListPlan {
                        query: plan,
                        mentions: mentions.label(),
                        limit: request.limit,
                        scan_limit: request.scan_limit,
                        identity_resolution: if mentions
                            == crate::discord::mention_target::MentionTarget::AuthenticatedBot
                        {
                            "requires-live-users/@me"
                        } else {
                            "explicit-user-id"
                        },
                    },
                )?;
            } else {
                context.emit_plan(&plan)?;
            }
            return Ok(());
        }
        let mut purpose = format!(
            "{}; limit {}; before {:?}",
            plan.operation, request.limit, self.before
        );
        if let Some(mentions) = request.mentions {
            write!(
                purpose,
                "; mentions {}; scan limit {}; authenticated bot identity when me",
                mentions.label(),
                request.scan_limit
            )?;
        }
        let http = context
            .http(&purpose, plan.guild_id, plan.channel_id)
            .await?;
        http.run(context.check_live_target(&http, &plan)).await?;
        if request.mentions.is_some() {
            let report = crate::discord::message_mentions::scan(&http, &request).await?;
            http.check()?;
            crate::json_stdout::print_serde_json(&report)
        } else {
            let messages = http
                .page(request.channel_id, request.before, request.limit)
                .await
                .wrap_err("Failed to list channel messages")?;
            http.check()?;
            crate::json_stdout::print_serde_json(&messages)
        }
    }
}
