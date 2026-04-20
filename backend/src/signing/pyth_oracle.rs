//! Pyth price oracle for SOL/USD pricing via Hermes API.
//!
//! Fetches SOL/USD price from Pyth's off-chain Hermes API every 5 minutes
//! and provides GBP→lamports conversion for dynamic fee calculation.

use serde::Deserialize;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct PythPriceResponse {
    data: Vec<PythPriceData>,
}

#[derive(Debug, Deserialize)]
struct PythPriceData {
    id: String,
    price: PythPrice,
}

#[derive(Debug, Deserialize)]
struct PythPrice {
    price: String,
}

/// Pyth oracle for fetching and caching SOL/USD prices
pub struct PythOracle {
    sol_usd_price: RwLock<Option<f64>>,
}

impl PythOracle {
    pub fn new() -> Self {
        Self {
            sol_usd_price: RwLock::new(None),
        }
    }

    /// Fetch SOL/USD price from Pyth Hermes API
    pub async fn fetch_price(&self) -> Result<f64, String> {
        // SOL/USD feed ID from Pyth
        let url = "https://hermes.pyth.network/v2/updates/price/latest?ids=0xef0dabb2aa495fd0e110d4fafd19c38069fe95f972e3035425d35c0e4478efdb";
        
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
        
        let response = client.get(url).send().await
            .map_err(|e| format!("HTTP request failed: {}", e))?;
        
        let data: PythPriceResponse = response.json().await
            .map_err(|e| format!("JSON parse failed: {}", e))?;
        
        if data.data.is_empty() {
            return Err("No price data returned from Pyth".to_string());
        }
        
        // Pyth returns price as string like "123.45"
        let price_str = &data.data[0].price.price;
        let price: f64 = price_str.parse()
            .map_err(|e| format!("Price parse failed: {}", e))?;
        
        // Update cache
        *self.sol_usd_price.write().unwrap() = Some(price);
        
        Ok(price)
    }

    /// Convert GBP to lamports using current SOL price
    /// Approximate GBP→USD rate of 1.25
    pub fn gbp_to_lamports(&self, gbp: f64) -> Option<u64> {
        let sol_price = *self.sol_usd_price.read().unwrap();
        let sol_price = sol_price?;
        let gbp_to_usd = gbp * 1.25; // Approximate GBP→USD rate
        let sol_amount = gbp_to_usd / sol_price;
        Some((sol_amount * 1_000_000_000.0) as u64)
    }

    /// Get cached price if available
    pub fn get_cached_price(&self) -> Option<f64> {
        *self.sol_usd_price.read().unwrap()
    }
}

impl Default for PythOracle {
    fn default() -> Self {
        Self::new()
    }
}
