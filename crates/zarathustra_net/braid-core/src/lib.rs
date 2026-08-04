//! braid-core: the thin Braid-HTTP protocol surface used by XFChess.
//!
//! Re-exports the protocol types and client from `braid-http` plus the local
//! error type, behind one stable import path so the rest of `zarathustra_net`
//! depends on this crate rather than `braid-http` directly. XFChess is
//! server-authoritative (append-log for moves, JSON Patch for tournament
//! documents), so this crate carries only protocol vocabulary and the client
//! path — no filesystem sync, blob store, or CRDT merge. See the crate README.

pub mod core;

// Top-level re-exports for common usage
pub use crate::core::error::{BraidError, Result};
pub use crate::core::{BraidRequest, BraidResponse, Patch, Update, Version};
pub use braid_http::types;

#[cfg(feature = "client")]
pub use crate::core::{BraidClient, ClientConfig, Subscription};
#[cfg(feature = "client")]
pub use braid_http::client;
