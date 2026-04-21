//! Canada CACF compliance requirements.

use serde::{Deserialize, Serialize};
use super::types::CacfComplianceStatus;

/// Canada CACF compliance requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanadaCompliance {
    /// Social Insurance Number (SIN) verified
    pub sin_verified: bool,
    /// FINTRAC LVCTR (Large Virtual Currency Transaction Report) enabled
    pub lvctr_enabled: bool,
    /// CRA reporting enabled
    pub cra_reporting_enabled: bool,
    /// Last LVCTR report date
    pub last_lvctr_report: Option<String>,
    /// Compliance status
    pub status: CacfComplianceStatus,
}

impl CanadaCompliance {
    /// Creates a new Canada compliance record
    pub fn new() -> Self {
        Self {
            sin_verified: false,
            lvctr_enabled: false,
            cra_reporting_enabled: false,
            last_lvctr_report: None,
            status: CacfComplianceStatus::NotCompliant,
        }
    }

    /// Updates compliance status based on requirements
    pub fn update_status(&mut self) {
        self.status = if self.sin_verified {
            CacfComplianceStatus::FullyCompliant
        } else {
            CacfComplianceStatus::NotCompliant
        };
    }

    /// Checks if LVCTR reporting is required for a transaction
    pub fn requires_lvctr(&self, transaction_amount_cad: f64) -> bool {
        // Canada requires LVCTR for transactions >$10,000
        transaction_amount_cad >= 10000.0 && self.status == CacfComplianceStatus::FullyCompliant
    }
}

impl Default for CanadaCompliance {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canada_lvctr() {
        let mut canada = CanadaCompliance::new();
        canada.sin_verified = true;
        canada.update_status();

        assert!(!canada.requires_lvctr(5000.0));
        assert!(canada.requires_lvctr(15000.0));
    }
}
