use crate::discord::leased_http::LeasedHttp;
use crate::watch_daemon::CaptureSignal;
use crate::watcher::WatcherScope;
use serenity::all::Context;
use serenity::all::EventHandler;
use serenity::all::Message;
use serenity::all::Ready;
use serenity::all::ResumedEvent;
use serenity::async_trait;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tokio::sync::mpsc;

/// Stateless wake projection. Raw Gateway content never enters durable storage.
pub(super) struct GatewayHandler {
    pub(super) http: Arc<LeasedHttp>,
    pub(super) scope: WatcherScope,
    pub(super) cancelled: Arc<AtomicBool>,
    pub(super) invalid_identity: Arc<AtomicBool>,
    pub(super) signals: mpsc::Sender<CaptureSignal>,
}

impl std::fmt::Debug for GatewayHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GatewayHandler([redacted connection])")
    }
}

impl GatewayHandler {
    fn active(&self) -> bool {
        !self.cancelled.load(Ordering::Acquire)
            && !self.invalid_identity.load(Ordering::Acquire)
            && self.http.check().is_ok()
    }

    pub(super) fn dirty(&self) {
        if self.active() {
            // A full slot already promises a sweep. Never allocate one message
            // per incoming event or block Serenity's callbacks behind capture.
            let _ = self.signals.try_send(CaptureSignal::Dirty);
        }
    }

    fn selected_message(&self, message: &Message) -> bool {
        message.channel_id.get() == self.scope.channel_id.get()
            && message
                .guild_id
                .is_some_and(|guild| guild.get() == self.scope.guild_id.get())
    }
}

#[async_trait]
impl EventHandler for GatewayHandler {
    async fn ready(&self, _context: Context, data_about_bot: Ready) {
        if !data_about_bot.user.bot || data_about_bot.user.id.get() != self.scope.bot_id.get() {
            self.invalid_identity.store(true, Ordering::Release);
            return;
        }
        if !self.active() {
            return;
        }
        tracing::info!(
            event_kind = "gateway-ready",
            authenticated_bot_id = data_about_bot.user.id.get(),
            selected_guild_id = self.scope.guild_id.get(),
            selected_channel_id = self.scope.channel_id.get(),
            "Verified watcher Gateway ready"
        );
        self.dirty();
    }

    async fn resume(&self, _context: Context, _event: ResumedEvent) {
        if !self.active() {
            return;
        }
        tracing::info!(
            event_kind = "gateway-resumed",
            authenticated_bot_id = self.scope.bot_id.get(),
            selected_guild_id = self.scope.guild_id.get(),
            selected_channel_id = self.scope.channel_id.get(),
            "Verified watcher Gateway resumed"
        );
        self.dirty();
    }

    async fn message(&self, _context: Context, new_message: Message) {
        if self.selected_message(&new_message) {
            self.dirty();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GatewayHandler;
    use crate::discord::leased_http::LeasedHttp;
    use crate::watch_daemon::CaptureSignal;
    use crate::watcher::WatcherChannelId;
    use crate::watcher::WatcherGuildId;
    use crate::watcher::WatcherScope;
    use crate::watcher::WatcherUserId;
    use serenity::all::ChannelId;
    use serenity::all::GuildId;
    use serenity::all::Message;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn burst_wakes_coalesce_and_cancelled_callbacks_cannot_enqueue() {
        let (signals, mut receiver) = mpsc::channel(1);
        let handler = GatewayHandler {
            http: Arc::new(LeasedHttp::direct("synthetic-never-transmitted").unwrap()),
            scope: WatcherScope {
                guild_id: WatcherGuildId::new(10).unwrap(),
                channel_id: WatcherChannelId::new(20).unwrap(),
                author_id: WatcherUserId::new(30).unwrap(),
                bot_id: WatcherUserId::new(40).unwrap(),
            },
            cancelled: Arc::new(AtomicBool::new(false)),
            invalid_identity: Arc::new(AtomicBool::new(false)),
            signals,
        };
        for _ in 0..1_000 {
            handler.dirty();
        }
        assert!(matches!(receiver.recv().await, Some(CaptureSignal::Dirty)));
        receiver.try_recv().unwrap_err();
        handler.cancelled.store(true, Ordering::Release);
        handler.dirty();
        receiver.try_recv().unwrap_err();

        let mut message = Message::default();
        message.channel_id = ChannelId::new(20);
        message.guild_id = Some(GuildId::new(10));
        assert!(handler.selected_message(&message));
        message.guild_id = Some(GuildId::new(11));
        assert!(!handler.selected_message(&message));
        message.guild_id = None;
        assert!(!handler.selected_message(&message));
    }
}
