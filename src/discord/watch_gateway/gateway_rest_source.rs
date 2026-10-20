use super::guarded_capture::guarded_capture;
use crate::discord::leased_http::LeasedHttp;
use crate::discord::query_plan::QueryPlan;
use crate::discord::read_context::ReadContext;
use crate::discord::watch_runtime_error::WatchRuntimeError;
use crate::watch_daemon::DaemonCaptureSource;
use crate::watcher::WatcherError;
use crate::watcher::WatcherMessage;
use crate::watcher::WatcherMessageId;
use crate::watcher::WatcherReply;
use crate::watcher::WatcherScope;
use crate::watcher::WatcherSource;
use crate::watcher::WatcherUserId;
use serenity::all::ChannelId;
use serenity::all::Message;
use serenity::all::MessageId;
use serenity::all::MessageReferenceKind;
use serenity::http::MessagePagination;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Read-only live transport. A single HTTP client retains Serenity's rate-limit state.
/// Scope is verified using live channel metadata before any message becomes a trigger.
#[derive(Debug)]
pub struct GatewayRestSource {
    http: Arc<LeasedHttp>,
    scope: WatcherScope,
    cancelled: Arc<AtomicBool>,
}
impl GatewayRestSource {
    /// # Errors
    /// Rejects a different authenticated bot, scope/policy mismatch or failed live checks.
    pub async fn connect(
        http: Arc<LeasedHttp>,
        scope: WatcherScope,
        context: &ReadContext,
        plan: &QueryPlan,
        cancelled: Arc<AtomicBool>,
    ) -> eyre::Result<Self> {
        let source = Self {
            http,
            scope,
            cancelled,
        };
        Box::pin(source.read(context.check_live_target(&source.http, plan))).await?;
        let user = Box::pin(source.read(async {
            source
                .http
                .get_current_user()
                .await
                .map_err(eyre::Report::from)
        }))
        .await?;
        eyre::ensure!(
            user.bot && user.id.get() == source.scope.bot_id.get(),
            WatchRuntimeError::InvalidBot
        );
        Ok(source)
    }

    async fn read<T>(&self, operation: impl Future<Output = eyre::Result<T>>) -> eyre::Result<T> {
        self.check_lease()?;
        let result = guarded_capture(self.http.run(operation), &self.cancelled, || {
            self.check_lease()
        })
        .await;
        if result.as_ref().err().is_some_and(|error| {
            matches!(
                error.downcast_ref::<WatchRuntimeError>(),
                Some(WatchRuntimeError::Cancelled)
            )
        }) {
            return result;
        }
        self.check_lease()?;
        result.map_err(Self::classify_transport)
    }

    fn classify_transport(error: eyre::Report) -> eyre::Report {
        if error.downcast_ref::<WatchRuntimeError>().is_some() {
            return error;
        }
        if let Some(serenity::Error::Http(http_error)) = error.downcast_ref::<serenity::Error>() {
            if let Some(status) = http_error.status_code() {
                return if status.is_server_error() || status.as_u16() == 429 {
                    WatchRuntimeError::Transient.into()
                } else {
                    WatchRuntimeError::AccessRejected.into()
                };
            }
            return WatchRuntimeError::Transient.into();
        }
        // Membership and local policy failures retain their reviewed categories.
        // They are never retried or printed as raw HTTP bodies.
        error
    }

    fn convert(&self, message: Message) -> eyre::Result<WatcherMessage> {
        eyre::ensure!(
            message.channel_id.get() == self.scope.channel_id.get(),
            WatcherError::ScopeMismatch
        );
        eyre::ensure!(
            message
                .guild_id
                .is_none_or(|guild| guild.get() == self.scope.guild_id.get()),
            WatcherError::ScopeMismatch
        );
        let candidate = message.author.id.get() == self.scope.author_id.get()
            && !message.author.bot
            && message.webhook_id.is_none()
            && message
                .mentions
                .iter()
                .any(|user| user.id.get() == self.scope.bot_id.get());
        let reply_to = if let Some(reference) = &message.message_reference {
            if candidate {
                eyre::ensure!(
                    reference.kind == MessageReferenceKind::Default
                        && reference.channel_id.get() == self.scope.channel_id.get()
                        && reference
                            .guild_id
                            .is_none_or(|guild| guild.get() == self.scope.guild_id.get())
                        && reference.message_id.is_some(),
                    WatcherError::InvalidReply
                );
            }
            // Nonqualifying messages remain scan context without cross-channel reads.
            reference
                .message_id
                .map(|id| WatcherMessageId::new(id.get()))
                .transpose()?
        } else {
            None
        };
        Ok(WatcherMessage {
            id: WatcherMessageId::new(message.id.get())?,
            // REST message objects can omit guild_id. The live channel check above
            // establishes guild membership. A conflicting supplied guild is rejected.
            guild_id: self.scope.guild_id,
            channel_id: self.scope.channel_id,
            author_id: WatcherUserId::new(message.author.id.get())?,
            author_is_bot: message.author.bot,
            is_webhook: message.webhook_id.is_some(),
            mentioned_user_ids: message
                .mentions
                .iter()
                .map(|user| WatcherUserId::new(user.id.get()))
                .collect::<eyre::Result<_>>()?,
            content: message.content,
            reply_to,
            reply: None,
        })
    }

    async fn fetch_raw(&self, id: WatcherMessageId) -> eyre::Result<Message> {
        let raw = Box::pin(self.read(async {
            self.http
                .get_message(
                    ChannelId::new(self.scope.channel_id.get()),
                    MessageId::new(id.get()),
                )
                .await
                .map_err(eyre::Report::from)
        }))
        .await?;
        eyre::ensure!(raw.id.get() == id.get(), WatcherError::InvalidReply);
        self.verify_message_scope(&raw)?;
        Ok(raw)
    }

    async fn fetch(&self, id: WatcherMessageId) -> eyre::Result<WatcherMessage> {
        self.convert(self.fetch_raw(id).await?)
    }

    fn verify_message_scope(&self, message: &Message) -> eyre::Result<()> {
        eyre::ensure!(
            message.channel_id.get() == self.scope.channel_id.get(),
            WatcherError::ScopeMismatch
        );
        eyre::ensure!(
            message
                .guild_id
                .is_none_or(|guild| guild.get() == self.scope.guild_id.get()),
            WatcherError::ScopeMismatch
        );
        Ok(())
    }

    fn reply_from(&self, raw: Message, expected: WatcherMessageId) -> eyre::Result<WatcherReply> {
        eyre::ensure!(raw.id.get() == expected.get(), WatcherError::InvalidReply);
        self.verify_message_scope(&raw)?;
        // Reply context is not a trigger. Ignore its own reference, mention text,
        // forwarded-message kind and embedded reference. Never recurse into it.
        Ok(WatcherReply {
            id: expected,
            guild_id: self.scope.guild_id,
            channel_id: self.scope.channel_id,
            author_id: WatcherUserId::new(raw.author.id.get())?,
            content: raw.content,
        })
    }

    async fn hydrate(&self, message: &mut WatcherMessage) -> eyre::Result<()> {
        let candidate = message.author_id == self.scope.author_id
            && !message.author_is_bot
            && !message.is_webhook
            && message.mentioned_user_ids.contains(&self.scope.bot_id);
        if candidate && let Some(id) = message.reply_to {
            // Always fetch the exact reference. Embedded referenced_message and
            // quoted text cannot supply authenticated source metadata.
            message.reply = Some(self.reply_from(self.fetch_raw(id).await?, id)?);
        }
        Ok(())
    }
}
impl crate::watch_daemon::DaemonCaptureSource for GatewayRestSource {
    fn check_lease(&self) -> eyre::Result<()> {
        self.http
            .check()
            .map_err(|_redacted_error| eyre::eyre!(WatchRuntimeError::ApprovalRequired))
    }
}
impl WatcherSource for GatewayRestSource {
    async fn page(
        &mut self,
        before: Option<WatcherMessageId>,
        limit: u8,
    ) -> eyre::Result<Vec<WatcherMessage>> {
        let raw = Box::pin(self.read(async {
            self.http
                .get_messages(
                    ChannelId::new(self.scope.channel_id.get()),
                    before.map(|id| MessagePagination::Before(MessageId::new(id.get()))),
                    Some(limit),
                )
                .await
                .map_err(eyre::Report::from)
        }))
        .await?;
        let mut messages = Vec::with_capacity(raw.len());
        for raw_message in raw {
            let mut message = self.convert(raw_message)?;
            self.hydrate(&mut message).await?;
            messages.push(message);
        }
        Ok(messages)
    }

    async fn get(&mut self, id: WatcherMessageId) -> eyre::Result<WatcherMessage> {
        let mut message = self.fetch(id).await?;
        self.hydrate(&mut message).await?;
        Ok(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::watcher::WatcherChannelId;
    use crate::watcher::WatcherGuildId;
    use serenity::all::GuildId;
    use serenity::all::MessageReference;
    use serenity::all::User;
    use serenity::all::UserId;

    fn source() -> GatewayRestSource {
        GatewayRestSource {
            http: Arc::new(LeasedHttp::direct("synthetic-never-transmitted").unwrap()),
            scope: WatcherScope {
                guild_id: WatcherGuildId::new(10).unwrap(),
                channel_id: WatcherChannelId::new(20).unwrap(),
                author_id: WatcherUserId::new(30).unwrap(),
                bot_id: WatcherUserId::new(40).unwrap(),
            },
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
    fn raw() -> Message {
        let mut message = Message::default();
        message.id = MessageId::new(100);
        message.channel_id = ChannelId::new(20);
        message.guild_id = Some(GuildId::new(10));
        message.author.id = UserId::new(30);
        message.author.bot = false;
        let mut bot = User::default();
        bot.id = UserId::new(40);
        bot.bot = true;
        message.mentions = vec![bot];
        message.content = "Synthetic context".into();
        message
    }

    #[test]
    fn reply_projection_ignores_nested_reference_without_promoting_it_to_a_trigger() {
        let source = source();
        let mut context = raw();
        context.message_reference = Some(
            MessageReference::new(MessageReferenceKind::Forward, ChannelId::new(999))
                .message_id(MessageId::new(998))
                .guild_id(GuildId::new(997)),
        );
        let reply = source
            .reply_from(context.clone(), WatcherMessageId::new(100).unwrap())
            .unwrap();
        assert_eq!(reply.id.get(), 100);
        assert_eq!(reply.author_id.get(), 30);
        assert_eq!(reply.content, "Synthetic context");
        // The same unsupported reference remains invalid on the top-level trigger.
        assert!(matches!(
            source
                .convert(context)
                .unwrap_err()
                .downcast_ref::<WatcherError>(),
            Some(WatcherError::InvalidReply)
        ));
    }

    #[test]
    fn fetched_reply_requires_exact_id_channel_and_present_guild_metadata() {
        let source = source();
        let _ = source
            .reply_from(raw(), WatcherMessageId::new(101).unwrap())
            .unwrap_err();
        let mut wrong_channel = raw();
        wrong_channel.channel_id = ChannelId::new(999);
        let _ = source
            .reply_from(wrong_channel, WatcherMessageId::new(100).unwrap())
            .unwrap_err();
        let mut wrong_guild = raw();
        wrong_guild.guild_id = Some(GuildId::new(999));
        let _ = source
            .reply_from(wrong_guild, WatcherMessageId::new(100).unwrap())
            .unwrap_err();
        let mut omitted_guild = raw();
        omitted_guild.guild_id = None;
        // The constructor's live channel-membership check supplies omitted REST guild metadata.
        source
            .reply_from(omitted_guild, WatcherMessageId::new(100).unwrap())
            .unwrap();
    }

    #[test]
    fn textual_mentions_and_mentions_of_the_human_are_not_trigger_authority() {
        let source = source();
        let mut text_only = raw();
        text_only.mentions.clear();
        text_only.content = "<@40>".into();
        assert!(
            !source
                .convert(text_only)
                .unwrap()
                .is_trigger(&source.scope)
                .unwrap()
        );
        let mut other_author = raw();
        other_author.author.id = UserId::new(50);
        let mut human = User::default();
        human.id = UserId::new(30);
        other_author.mentions.push(human);
        assert!(
            !source
                .convert(other_author)
                .unwrap()
                .is_trigger(&source.scope)
                .unwrap()
        );
    }
}
