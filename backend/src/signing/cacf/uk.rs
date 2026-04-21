//! UK CACF compliance requirements.

use serde::{Deserialize, Serialize};
use super::types::CacfComplianceStatus;

/// UK CACF compliance requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UKCompliance {
    /// Full KYC completed
    pub kyc_completed: bool,
    /// National Insurance Number (NI) verified
    pub ni_verified: bool,
    /// Unique Taxpayer Reference (UTR) verified
    pub utr_verified: bool,
    /// Annual HMRC reporting enabled
    pub hmrc_reporting_enabled: bool,
    /// Last annual report year
    pub last_report_year: Option<u32>,
    /// Compliance status
    pub status: CacfComplianceStatus,
}

impl UKCompliance {
    /// Creates a new UK compliance record
    pub fn new() -> Self {
        Self {
            kyc_completed: false,
            ni_verified: false,
            utr_verified: false,
            hmrc_reporting_enabled: false,
            last_report_year: None,
            status: CacfComplianceStatus::NotCompliant,
        }
    }

    /// Updates compliance status based on requirements
    pub fn update_status(&mut self) {
        self.status = if self.kyc_completed && self.ni_verified && self.utr_verified {
            CacfComplianceStatus::FullyCompliant
        } else if self.kyc_completed || self.ni_verified || self.utr_verified {
            CacfComplianceStatus::PartiallyCompliant
        } else {
            CacfComplianceStatus::NotCompliant
        };
    }

    /// Checks if annual reporting is required for a given year
    ///
    /// # Arguments
    /// * `_year` - The year as u32
    /// * `annual_winnings_gbp` - Annual winnings in GBP
    ///
    /// # Returns
    /// True if threshold exceeded
    pub fn requires_reporting(&self, _year: u32, annual_winnings_gbp: f64) -> bool {
        // UK requires annual reporting for winnings >£10,000
        annual_winnings_gbp >= 10000.0 && self.status == CacfComplianceStatus::FullyCompliant
    }
}

impl Default for UKCompliance {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uk_compliance_status() {
        let mut uk = UKCompliance::new();
        assert_eq!(uk.status, CacfComplianceStatus::NotCompliant);

        uk.kyc_completed = true;
        uk.update_status();
        assert_eq!(uk.status, CacfComplianceStatus::PartiallyCompliant);

        uk.ni_verified = true;
        uk.utr_verified = true;
        uk.update_status();
        assert_eq!(uk.status, CacfComplianceStatus::FullyCompliant);
    }

    #[test]
    fn test_uk_requires_reporting() {
        let mut uk = UKCompliance::new();
        uk.kyc_completed = true;
        uk.ni_verified = true;
        uk.utr_verified = true;
        uk.update_status();

        assert!(!uk.requires_reporting(2024, 5000.0));
        assert!(uk.requires_reporting(2024, 15000.0));
    }
}
