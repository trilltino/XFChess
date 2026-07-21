//! Governance instructions for resolving disputes and game issues.

pub mod claim_stale_dispute;
pub mod dispute;
pub mod resolution;
pub mod resolve;

pub use claim_stale_dispute::ClaimStaleDispute;
pub use dispute::DisputeGame;
pub use resolve::ResolveDispute;
