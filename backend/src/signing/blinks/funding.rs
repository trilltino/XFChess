//! Funding integration for Solana Blinks onboarding.
//!
//! This module provides helper functions and endpoints for integrating with
//! MoonPay, Transak, and Banxa for SOL onboarding.

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Funding request parameters.
#[derive(Serialize, Deserialize)]
pub struct FundingRequest {
    pub wallet_address: String,
    pub amount_sol: f64,
    pub currency: String,
    pub provider: FundingProvider,
}

/// Funding provider options.
#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum FundingProvider {
    MoonPay,
    Transak,
    Banxa,
}

/// Funding response with redirect URL.
#[derive(Serialize, Deserialize)]
pub struct FundingResponse {
    pub redirect_url: String,
    pub provider: String,
    pub amount_sol: f64,
    pub currency: String,
}

/// Generates a funding URL for the specified provider.
pub fn generate_funding_url(
    wallet_address: &str,
    amount_sol: f64,
    currency: &str,
    provider: FundingProvider,
    api_key: &str,
) -> Result<String> {
    match provider {
        FundingProvider::MoonPay => {
            Ok(format!(
                "https://buy.moonpay.com?apiKey={}&currencyCode={}&walletAddress={}&amount={}",
                api_key, currency, wallet_address, amount_sol
            ))
        }
        FundingProvider::Transak => {
            Ok(format!(
                "https://transak.com/buy?apiKey={}&cryptoCurrency=SOL&fiatCurrency={}&walletAddress={}&amount={}",
                api_key, currency, wallet_address, amount_sol
            ))
        }
        FundingProvider::Banxa => {
            Ok(format!(
                "https://banxa.com/buy?apiKey={}&coin=SOL&fiat={}&wallet={}&amount={}",
                api_key, currency, wallet_address, amount_sol
            ))
        }
    }
}

/// Validates that a funding URL is properly formatted.
pub fn validate_funding_url(url: &str) -> bool {
    url.starts_with("https://") && (url.contains("moonpay.com") || url.contains("transak.com") || url.contains("banxa.com"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_moonpay_url() {
        let url = generate_funding_url("test_wallet", 0.5, "USD", FundingProvider::MoonPay, "test_key")
            .unwrap();
        assert!(url.contains("moonpay.com"));
        assert!(url.contains("walletAddress=test_wallet"));
        assert!(url.contains("amount=0.5"));
    }

    #[test]
    fn test_generate_transak_url() {
        let url = generate_funding_url("test_wallet", 0.5, "USD", FundingProvider::Transak, "test_key")
            .unwrap();
        assert!(url.contains("transak.com"));
        assert!(url.contains("walletAddress=test_wallet"));
    }

    #[test]
    fn test_generate_banxa_url() {
        let url = generate_funding_url("test_wallet", 0.5, "USD", FundingProvider::Banxa, "test_key")
            .unwrap();
        assert!(url.contains("banxa.com"));
        assert!(url.contains("wallet=test_wallet"));
    }

    #[test]
    fn test_validate_funding_url() {
        assert!(validate_funding_url("https://buy.moonpay.com?test=1"));
        assert!(validate_funding_url("https://transak.com/buy"));
        assert!(validate_funding_url("https://banxa.com/buy"));
        assert!(!validate_funding_url("http://invalid.com"));
    }
}
