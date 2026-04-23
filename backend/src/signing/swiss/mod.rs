//! Swiss Tournament System Module
//!
//! Provides Swiss-system tournament management with real-time
//! pairing generation via the braid-iroh networking stack.

pub mod service;
pub mod handlers;
pub mod orchestrator;

pub use handlers::*;
pub use orchestrator::{OrchestratorEvent, spawn_orchestrator};
pub use service::*;
