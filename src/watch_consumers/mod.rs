//! Durable named consumers over an immutable, deduplicated Discord event log.
//! The daemon actor owns this store; clients never open its snapshot directly.

mod consumer_acknowledgment;
mod consumer_claim;
mod consumer_claim_result;
mod consumer_delivery;
mod consumer_delivery_id;
mod consumer_error;
mod consumer_event;
mod consumer_name;
mod consumer_operation;
mod consumer_provenance;
mod consumer_publish_report;
mod consumer_registration;
mod consumer_request_binding;
mod consumer_request_id;
mod consumer_request_record;
mod consumer_start;
mod consumer_state;
mod consumer_status;
mod consumer_store;

pub use consumer_acknowledgment::ConsumerAcknowledgment;
pub use consumer_claim::ConsumerClaim;
pub use consumer_claim_result::ConsumerClaimResult;
pub use consumer_delivery::ConsumerDelivery;
pub use consumer_delivery_id::ConsumerDeliveryId;
pub use consumer_error::ConsumerError;
pub use consumer_event::ConsumerEvent;
pub use consumer_name::ConsumerName;
pub use consumer_operation::ConsumerOperation;
pub use consumer_provenance::ConsumerProvenance;
pub use consumer_publish_report::ConsumerPublishReport;
pub use consumer_registration::ConsumerRegistration;
pub use consumer_request_binding::ConsumerRequestBinding;
pub use consumer_request_id::ConsumerRequestId;
pub use consumer_request_record::ConsumerRequestRecord;
pub use consumer_start::ConsumerStart;
pub use consumer_state::ConsumerState;
pub use consumer_status::ConsumerStatus;
pub use consumer_store::ConsumerStore;
