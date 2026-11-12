//! Transport-neutral network protocol and signed wire messages.

pub mod backend;
pub mod profile_identity;
pub mod protocol;
pub mod reorder;

pub use protocol::{calculate_batch_hash, NetworkMessage, SignedNetworkMessage};
pub use reorder::{IngestOutcome, NonceSequencer};
