//! Storage modules for the signing service.
//!
//! This module provides SQLite-backed storage for:
//! - Game sessions (session keys)
//! - Tournaments (bracket management)
//! - KYC vault (GDPR-compliant PII storage)

pub mod session;
pub mod tournament;
pub mod vault;

pub use session::{SessionEntry, SessionStore};
pub use vault::{KycRecord, VaultStore};
