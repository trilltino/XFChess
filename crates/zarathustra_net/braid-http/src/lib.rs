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

pub use client::BraidClient;
pub use types::{BraidRequest, BraidResponse};
// Version might be in a submodule of types or directly in types/mod.rs
pub use error::{BraidError, Result};
/// Wire format: header parsing/emission, the `209` stream state machine, update framing.
pub mod protocol;
