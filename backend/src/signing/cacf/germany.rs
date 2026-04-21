//! Germany CACF compliance requirements.

use serde::{Deserialize, Serialize};
use super::types::CacfComplianceStatus;

/// Germany CACF compliance requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GermanyCompliance {
    /// Residency proof verified
    pub residency_verified: bool,
    /// Tax ID verified
    pub tax_id_verified: bool,
    /// Monthly tracking of €999.99 limit
    pub monthly_limit_tracking_enabled: bool,
    /// Monthly total winnings in EUR
    pub monthly_winnings_eur: f64,
    /// Last month tracked
    pub last_tracked_month: Option<String>, // Format: "2024-03"
    /// Compliance status
    pub status: CacfComplianceStatus,
}

impl GermanyCompliance {
    /// Creates a new Germany compliance record
    pub fn new() -> Self {
        Self {
            residency_verified: false,
            tax_id_verified: false,
            monthly_limit_tracking_enabled: false,
            monthly_winnings_eur: 0.0,
            last_tracked_month: None,
            status: CacfComplianceStatus::NotCompliant,
        }
    }

    /// Updates compliance status based on requirements
    pub fn update_status(&mut self) {
        self.status = if self.residency_verified && self.tax_id_verified {
            CacfComplianceStatus::FullyCompliant
        } else if self.residency_verified || self.tax_id_verified {
            CacfComplianceStatus::PartiallyCompliant
        } else {
            CacfComplianceStatus::NotCompliant
        };
    }

    /// Checks if the €999.99 monthly limit is exceeded
    pub fn exceeds_monthly_limit(&self, year_month: &str) -> bool {
        if let Some(ref last_month) = self.last_tracked_month {
            if last_month == year_month {
                return self.monthly_winnings_eur > 999.99;
            }
        }
        false
    }

    /// Adds winnings to monthly total
    pub fn add_monthly_winnings(&mut self, year_month: &str, amount_eur: f64) {
        if let Some(ref last_month) = self.last_tracked_month {
            if last_month != year_month {
                // Reset for new month
                self.monthly_winnings_eur = 0.0;
            }
        }
        self.monthly_winnings_eur += amount_eur;
        self.last_tracked_month = Some(year_month.to_string());
    }
}

impl Default for GermanyCompliance {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_germany_monthly_limit() {
        let mut germany = GermanyCompliance::new();
        germany.add_monthly_winnings("2024-03", 500.0);
        assert!(!germany.exceeds_monthly_limit("2024-03"));

        germany.add_monthly_winnings("2024-03", 600.0);
        assert!(germany.exceeds_monthly_limit("2024-03"));
    }

    #[test]
    fn test_germany_monthly_reset() {
        let mut germany = GermanyCompliance::new();
        germany.add_monthly_winnings("2024-03", 500.0);
        assert_eq!(germany.monthly_winnings_eur, 500.0);

        germany.add_monthly_winnings("2024-04", 300.0);
        assert_eq!(germany.monthly_winnings_eur, 300.0); // Reset for new month
    }
}
