//! Rust implementation of the Braid-HTTP protocol (HTTP `209 Subscription`):
//! turns a resource into a live, versioned stream — one `GET` returns the
//! current state and then stays open, pushing each subsequent change. The
//! protocol core of the `zarathustra_net` stack; every other `braid-*` crate
//! builds on the types and client defined here. See the crate README for the
//! wire protocol and module map.

/// The portable Braid client: transport backends, one-shot fetch, and
/// long-lived subscriptions with heartbeat liveness.
pub mod client;
pub mod error;
/// The `BraidNetwork` transport abstraction the client dispatches through.
pub mod traits;
/// The data model: `Version`, `Patch`, `Update`, request/response builders.
pub mod types;

/// Wire format: header parsing/emission, the `209` stream state machine, update framing.
pub mod protocol;
/// Server-side helpers for answering a subscription. Transport-neutral — no
/// framework dependency, just headers and bytes.
pub mod server;

pub use client::{BraidClient, HeartbeatConfig, ReliableChannel, Subscription};
pub use error::{BraidError, Result};
pub use types::{BraidRequest, BraidResponse, Patch, Update, Version};

/// Everything needed to use this crate, in one import.
///
/// ```
/// use braid_http::prelude::*;
/// ```
pub mod prelude {
    pub use crate::client::{BraidClient, HeartbeatConfig, ReliableChannel, Subscription};
    pub use crate::error::{BraidError, Result};
    pub use crate::protocol::formatter::{format_heartbeat, format_update};
    pub use crate::server::SubscriptionResponse;
    pub use crate::types::{BraidRequest, BraidResponse, Patch, Update, Version};
}
