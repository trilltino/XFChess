//! Shared types for CACF compliance.

use serde::{Deserialize, Serialize};

/// CACF compliance status for a user
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CacfComplianceStatus {
    /// User is not compliant
    NotCompliant,
    /// Partially compliant (some requirements met)
    PartiallyCompliant,
    /// Fully compliant
    FullyCompliant,
    /// Under review
    UnderReview,
    /// Suspended
    Suspended,
}
