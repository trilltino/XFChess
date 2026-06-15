//! braid-core: the thin Braid-HTTP protocol surface used by XFChess.
//!
//! Re-exports the protocol types and client from `braid-http` plus the local
//! error type. The historical filesystem-sync (`fs`), blob storage (`blob`),
//! server (`server`) and vendored CRDT (`vendor`/diamond-types, conflict
//! `merge`) modules were removed — XFChess only consumes the `Update`/`Version`
//! types and the Braid client path.

pub mod core;

// Top-level re-exports for common usage
pub use crate::core::error::{BraidError, Result};
pub use crate::core::{BraidRequest, BraidResponse, Patch, Update, Version};
pub use braid_http::types;

#[cfg(feature = "client")]
pub use crate::core::{BraidClient, ClientConfig, Subscription};
#[cfg(feature = "client")]
pub use braid_http::client;
