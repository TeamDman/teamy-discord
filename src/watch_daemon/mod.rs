//! A single local daemon actor owns capture publication and consumer claims.
//! Clients exchange bounded Facet frames and never receive credentials or write state.
mod actor;
mod actor_message;
mod capture_signal;
mod client;
mod connection_id;
mod daemon_capture_source;
mod daemon_command;
mod daemon_endpoint;
mod daemon_hello;
mod daemon_options;
mod daemon_outcome;
#[cfg(windows)]
mod daemon_peer_process;
mod daemon_request;
mod daemon_response;
mod daemon_status;
mod daemon_transport_task;
mod offline_source;
mod pending_wait;
#[cfg(windows)]
mod pipe_security;
mod pipe_server;
mod server;
mod wire;
pub use capture_signal::CaptureSignal;
pub use client::client_request;
pub use daemon_capture_source::DaemonCaptureSource;
pub use daemon_command::DaemonCommand;
pub use daemon_endpoint::DaemonEndpoint;
pub use daemon_hello::DaemonHello;
pub use daemon_options::DaemonOptions;
pub use daemon_outcome::DaemonOutcome;
pub use daemon_request::DaemonRequest;
pub use daemon_response::DaemonResponse;
pub use daemon_status::DaemonStatus;
pub use server::serve_offline;
pub use server::serve_with_capture;
pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;
