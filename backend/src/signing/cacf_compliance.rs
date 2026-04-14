//! CACF (Crypto Asset Compliance Framework) compliance implementation.
//!
//! This module handles country-specific compliance requirements for UK, Brazil,
//! Germany, and Canada including KYC verification, tax ID validation, and reporting.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

/// Brazil CACF compliance requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrazilCompliance {
    /// CPF (Cadastro de Pessoas Físicas) verified
    pub cpf_verified: bool,
    /// Monthly reporting enabled
    pub monthly_reporting_enabled: bool,
    /// Last monthly report month
    pub last_report_month: Option<String>, // Format: "2024-03"
    /// Compliance status
    pub status: CacfComplianceStatus,
}

impl BrazilCompliance {
    /// Creates a new Brazil compliance record
    pub fn new() -> Self {
        Self {
            cpf_verified: false,
            monthly_reporting_enabled: false,
            last_report_month: None,
            status: CacfComplianceStatus::NotCompliant,
        }
    }

    /// Updates compliance status based on requirements
    pub fn update_status(&mut self) {
        self.status = if self.cpf_verified {
            CacfComplianceStatus::FullyCompliant
        } else {
            CacfComplianceStatus::NotCompliant
        };
    }

    /// Checks if monthly reporting is required for a given month
    pub fn requires_reporting(&self, _year_month: &str, monthly_winnings_brl: f64) -> bool {
        // Brazil requires monthly reporting for winnings >R$30,000
        monthly_winnings_brl >= 30000.0 && self.status == CacfComplianceStatus::FullyCompliant
    }
}

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

/// CACF compliance manager for all jurisdictions
pub struct CacfComplianceManager {
    /// UK compliance records by wallet
    uk_records: HashMap<String, UKCompliance>,
    /// Brazil compliance records by wallet
    brazil_records: HashMap<String, BrazilCompliance>,
    /// Germany compliance records by wallet
    germany_records: HashMap<String, GermanyCompliance>,
    /// Canada compliance records by wallet
    canada_records: HashMap<String, CanadaCompliance>,
}

impl Default for CacfComplianceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CacfComplianceManager {
    /// Creates a new CACF compliance manager
    pub fn new() -> Self {
        Self {
            uk_records: HashMap::new(),
            brazil_records: HashMap::new(),
            germany_records: HashMap::new(),
            canada_records: HashMap::new(),
        }
    }

    /// Gets or creates UK compliance record for a wallet
    pub fn get_uk_compliance(&mut self, wallet: &str) -> &mut UKCompliance {
        self.uk_records.entry(wallet.to_string()).or_insert_with(UKCompliance::new)
    }

    /// Gets or creates Brazil compliance record for a wallet
    pub fn get_brazil_compliance(&mut self, wallet: &str) -> &mut BrazilCompliance {
        self.brazil_records.entry(wallet.to_string()).or_insert_with(BrazilCompliance::new)
    }

    /// Gets or creates Germany compliance record for a wallet
    pub fn get_germany_compliance(&mut self, wallet: &str) -> &mut GermanyCompliance {
        self.germany_records.entry(wallet.to_string()).or_insert_with(GermanyCompliance::new)
    }

    /// Gets or creates Canada compliance record for a wallet
    pub fn get_canada_compliance(&mut self, wallet: &str) -> &mut CanadaCompliance {
        self.canada_records.entry(wallet.to_string()).or_insert_with(CanadaCompliance::new)
    }

    /// Gets compliance status for a wallet and country
    pub fn get_compliance_status(&self, wallet: &str, country_code: &str) -> CacfComplianceStatus {
        match country_code {
            "GB" => self.uk_records.get(wallet).map(|c| c.status.clone()).unwrap_or(CacfComplianceStatus::NotCompliant),
            "BR" => self.brazil_records.get(wallet).map(|c| c.status.clone()).unwrap_or(CacfComplianceStatus::NotCompliant),
            "DE" => self.germany_records.get(wallet).map(|c| c.status.clone()).unwrap_or(CacfComplianceStatus::NotCompliant),
            "CA" => self.canada_records.get(wallet).map(|c| c.status.clone()).unwrap_or(CacfComplianceStatus::NotCompliant),
            _ => CacfComplianceStatus::FullyCompliant, // Other countries don't require compliance
        }
    }

    /// Checks if a wallet can participate in wager games based on compliance
    pub fn can_participate_in_wagers(&self, wallet: &str, country_code: &str) -> bool {
        let status = self.get_compliance_status(wallet, country_code);
        matches!(status, CacfComplianceStatus::FullyCompliant | CacfComplianceStatus::PartiallyCompliant)
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

    #[test]
    fn test_brazil_compliance_status() {
        let mut brazil = BrazilCompliance::new();
        assert_eq!(brazil.status, CacfComplianceStatus::NotCompliant);

        brazil.cpf_verified = true;
        brazil.update_status();
        assert_eq!(brazil.status, CacfComplianceStatus::FullyCompliant);
    }

    #[test]
    fn test_brazil_requires_reporting() {
        let mut brazil = BrazilCompliance::new();
        brazil.cpf_verified = true;
        brazil.update_status();

        assert!(!brazil.requires_reporting("2024-03", 20000.0));
        assert!(brazil.requires_reporting("2024-03", 35000.0));
    }

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

    #[test]
    fn test_canada_lvctr() {
        let mut canada = CanadaCompliance::new();
        canada.sin_verified = true;
        canada.update_status();

        assert!(!canada.requires_lvctr(5000.0));
        assert!(canada.requires_lvctr(15000.0));
    }

    #[test]
    fn test_compliance_manager() {
        let mut manager = CacfComplianceManager::new();

        let uk = manager.get_uk_compliance("wallet1");
        uk.kyc_completed = true;
        uk.ni_verified = true;
        uk.utr_verified = true;
        uk.update_status();

        let status = manager.get_compliance_status("wallet1", "GB");
        assert_eq!(status, CacfComplianceStatus::FullyCompliant);

        assert!(manager.can_participate_in_wagers("wallet1", "GB"));
    }
}
