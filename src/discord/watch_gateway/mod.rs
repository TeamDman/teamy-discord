//! Gateway wakeups with read-only REST catch-up under one existing broker lease.
//!
//! Gateway events are hints, never authoritative trigger content. The daemon's
//! single capture actor owns durable sweeps, outbox publication and ordering.
//! The caller must arm its whole-process lease watchdog before connecting:
//! Serenity retains credential copies in background tasks, so cooperative shard
//! shutdown alone is not a hard credential-lifetime boundary.

mod gateway_capture;
mod gateway_handler;
mod gateway_rest_source;
mod gateway_task;
mod guarded_capture;

pub use gateway_capture::GatewayCapture;
pub use gateway_rest_source::GatewayRestSource;
pub use gateway_task::GatewayTask;
pub use guarded_capture::guarded_capture;
