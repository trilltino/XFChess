//! Brazil CACF compliance requirements.

use super::types::CacfComplianceStatus;
use serde::{Deserialize, Serialize};

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

impl Default for BrazilCompliance {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
