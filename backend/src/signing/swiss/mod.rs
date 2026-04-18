//! Swiss Tournament System Module
//!
//! Provides Swiss-system tournament management with real-time
//! pairing generation via the braid-iroh networking stack.

pub mod handlers;
pub mod service;

pub use handlers::*;
pub use service::*;
