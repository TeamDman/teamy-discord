//! Durable, read-only Discord watcher state and a local acknowledged outbox.
//!
//! The caller owns authentication, transport, scheduling, dispatch and stdout.
//! A page and its outbox additions share one atomic snapshot. External dispatch
//! is at-least-once until acknowledged; use the message ID as an idempotency key.

mod watch_outbox_entry;
mod watch_provenance;
mod watch_state_store;
mod watch_sweep_report;
mod watch_sweep_request;
mod watch_sweep_resume;
mod watcher_channel_id;
mod watcher_error;
mod watcher_guild_id;
mod watcher_message;
mod watcher_message_id;
mod watcher_reply;
mod watcher_scope;
mod watcher_source;
mod watcher_state;
mod watcher_user_id;

pub use watch_outbox_entry::WatchOutboxEntry;
pub use watch_provenance::WatchProvenance;
pub use watch_state_store::WatchStateStore;
pub use watch_sweep_report::WatchSweepReport;
pub use watch_sweep_request::WatchSweepRequest;
pub use watch_sweep_resume::WatchSweepResume;
pub use watcher_channel_id::WatcherChannelId;
pub use watcher_error::WatcherError;
pub use watcher_guild_id::WatcherGuildId;
pub use watcher_message::WatcherMessage;
pub use watcher_message_id::WatcherMessageId;
pub use watcher_reply::WatcherReply;
pub use watcher_scope::WatcherScope;
pub use watcher_source::WatcherSource;
pub use watcher_state::WatcherState;
pub use watcher_user_id::WatcherUserId;
