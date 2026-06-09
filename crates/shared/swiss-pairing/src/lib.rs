//! Swiss Pairing Engine - FIDE Dutch System Implementation
//!
//! This crate provides a complete Swiss-system tournament pairing engine
//! implementing the FIDE Dutch system with color balancing.

pub mod color;
pub mod error;
pub mod pairing;
pub mod standings;
pub mod types;

pub use color::*;
pub use error::*;
pub use standings::*;
pub use types::*;

/// Re-export core functions for convenience
pub use pairing::generate_pairings;
pub use standings::calculate_standings;
pub use types::{ManualPairing, PairingConfig};
