//! CACF (Crypto Asset Compliance Framework) compliance implementation.
//!
//! This module handles country-specific compliance requirements for UK, Brazil,
//! Germany, and Canada including KYC verification, tax ID validation, and reporting.

pub mod brazil;
pub mod canada;
pub mod germany;
pub mod types;
pub mod uk;

pub use brazil::BrazilCompliance;
pub use canada::CanadaCompliance;
pub use germany::GermanyCompliance;
pub use types::CacfComplianceStatus;
pub use uk::UKCompliance;

use std::collections::HashMap;

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
        self.uk_records
            .entry(wallet.to_string())
            .or_insert_with(UKCompliance::new)
    }

    /// Gets or creates Brazil compliance record for a wallet
    pub fn get_brazil_compliance(&mut self, wallet: &str) -> &mut BrazilCompliance {
        self.brazil_records
            .entry(wallet.to_string())
            .or_insert_with(BrazilCompliance::new)
    }

    /// Gets or creates Germany compliance record for a wallet
    pub fn get_germany_compliance(&mut self, wallet: &str) -> &mut GermanyCompliance {
        self.germany_records
            .entry(wallet.to_string())
            .or_insert_with(GermanyCompliance::new)
    }

    /// Gets or creates Canada compliance record for a wallet
    pub fn get_canada_compliance(&mut self, wallet: &str) -> &mut CanadaCompliance {
        self.canada_records
            .entry(wallet.to_string())
            .or_insert_with(CanadaCompliance::new)
    }

    /// Gets compliance status for a wallet and country
    pub fn get_compliance_status(&self, wallet: &str, country_code: &str) -> CacfComplianceStatus {
        match country_code {
            "GB" => self
                .uk_records
                .get(wallet)
                .map(|c| c.status.clone())
                .unwrap_or(CacfComplianceStatus::NotCompliant),
            "BR" => self
                .brazil_records
                .get(wallet)
                .map(|c| c.status.clone())
                .unwrap_or(CacfComplianceStatus::NotCompliant),
            "DE" => self
                .germany_records
                .get(wallet)
                .map(|c| c.status.clone())
                .unwrap_or(CacfComplianceStatus::NotCompliant),
            "CA" => self
                .canada_records
                .get(wallet)
                .map(|c| c.status.clone())
                .unwrap_or(CacfComplianceStatus::NotCompliant),
            _ => CacfComplianceStatus::FullyCompliant, // Other countries don't require compliance
        }
    }

    /// Checks if a wallet can participate in wager games based on compliance
    pub fn can_participate_in_wagers(&self, wallet: &str, country_code: &str) -> bool {
        let status = self.get_compliance_status(wallet, country_code);
        matches!(
            status,
            CacfComplianceStatus::FullyCompliant | CacfComplianceStatus::PartiallyCompliant
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// Restricted jurisdictions must DEFAULT-DENY: a wallet with no compliance
    /// record on file cannot wager. This is the legally-critical edge — a
    /// regression flipping the default to allow would let unverified users in
    /// restricted countries wager.
    #[test]
    fn restricted_countries_default_deny_without_record() {
        let manager = CacfComplianceManager::new();
        for country in ["GB", "BR", "DE", "CA"] {
            assert_eq!(
                manager.get_compliance_status("unknown-wallet", country),
                CacfComplianceStatus::NotCompliant,
                "{country}: no record must be NotCompliant"
            );
            assert!(
                !manager.can_participate_in_wagers("unknown-wallet", country),
                "{country}: a wallet with no record must NOT be able to wager"
            );
        }
    }

    /// Non-restricted jurisdictions default-allow (no CACF regime applies).
    #[test]
    fn unrestricted_countries_default_allow() {
        let manager = CacfComplianceManager::new();
        for country in ["US", "FR", "JP", "ZZ"] {
            assert_eq!(
                manager.get_compliance_status("any-wallet", country),
                CacfComplianceStatus::FullyCompliant,
                "{country}: non-restricted country should be FullyCompliant"
            );
            assert!(manager.can_participate_in_wagers("any-wallet", country));
        }
    }
}
