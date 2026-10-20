use super::leased_http::LeasedHttp;
use super::mention_identity::MentionIdentity;
use super::message_page_source::MessagePageSource;
impl MessagePageSource for LeasedHttp {
    async fn current_user(&self) -> eyre::Result<MentionIdentity> {
        let user = self
            .run(async { self.get_current_user().await.map_err(eyre::Report::from) })
            .await?;
        Ok(MentionIdentity {
            id: user.id.get(),
            username: user.name.clone(),
            bot: user.bot,
        })
    }
    async fn page(
        &self,
        channel: u64,
        before: Option<u64>,
        limit: u8,
    ) -> eyre::Result<Vec<serenity::all::Message>> {
        let mut builder = serenity::all::GetMessages::new().limit(limit);
        if let Some(id) = before {
            builder = builder.before(serenity::all::MessageId::new(id));
        }
        self.run(async {
            serenity::all::ChannelId::new(channel)
                .messages(&**self, builder)
                .await
                .map_err(eyre::Report::from)
        })
        .await
    }
}
