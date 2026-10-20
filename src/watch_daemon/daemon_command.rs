use crate::watch_consumers::ConsumerDeliveryId;
use crate::watch_consumers::ConsumerName;
use crate::watch_consumers::ConsumerStart;
use crate::watcher::WatcherMessageId;
#[derive(Clone, Debug, facet::Facet)]
#[repr(u8)]
pub enum DaemonCommand {
    Next(ConsumerName),
    Status,
    CreateConsumer {
        consumer: ConsumerName,
        start: ConsumerStart,
    },
    Inspect(ConsumerName),
    Replay {
        consumer: ConsumerName,
        delivery_id: ConsumerDeliveryId,
    },
    Ack {
        consumer: ConsumerName,
        event_id: WatcherMessageId,
        delivery_id: ConsumerDeliveryId,
        receipt: String,
    },
    Stop,
}
