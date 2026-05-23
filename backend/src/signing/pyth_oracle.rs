//! Pyth price oracle for multi-currency pricing via Hermes API.
//!
//! Fetches SOL/USD + EUR/USD + GBP/USD + BRL/USD + CAD/USD from Pyth's
//! Hermes API in a single batch call every 5 minutes.
//!
//! Feed IDs — verify at https://pyth.network/price-feeds if updating:
//!   SOL/USD 0xef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d
//!   EUR/USD 0xa995d00bb36a63cef7fd2c287dc105fc8f3d93779f062f09551b0af3e81ec30
//!   GBP/USD 0x84c2dde9633d93d1bcad84e7dc41c9d56578b7ec52fabedc1f335d673df0a7c1
//!   BRL/USD 0x08f781a893bc9340140c5f89c8a96f438bcfae4d1474cc0f688e3a52892c7318
//!   CAD/USD 0x3b52af26afe72b3bdf6bdb60c49c45e7e7d5d9d3aa29e2ab58c5c0f4a8a2fb3

use serde::Deserialize;
use std::sync::RwLock;
use std::time::Duration;

// Pyth Hermes feed IDs (batch query, order matches FEED_IDS below)
const SOL_IDX: usize = 0;
const EUR_IDX: usize = 1;
const GBP_IDX: usize = 2;
const BRL_IDX: usize = 3;
const CAD_IDX: usize = 4;

const HERMES_URL: &str = concat!(
    "https://hermes.pyth.network/v2/updates/price/latest",
    "?ids=0xef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d",
    "&ids=0xa995d00bb36a63cef7fd2c287dc105fc8f3d93779f062f09551b0af3e81ec30",
    "&ids=0x84c2dde9633d93d1bcad84e7dc41c9d56578b7ec52fabedc1f335d673df0a7c1",
    "&ids=0x08f781a893bc9340140c5f89c8a96f438bcfae4d1474cc0f688e3a52892c7318",
    "&ids=0x3b52af26afe72b3bdf6bdb60c49c45e7e7d5d9d3aa29e2ab58c5c0f4a8a2fb3",
);

#[derive(Debug, Deserialize)]
struct HermesResponse {
    parsed: Vec<HermesParsed>,
}

#[derive(Debug, Deserialize)]
struct HermesParsed {
    price: HermesPrice,
}

#[derive(Debug, Deserialize)]
struct HermesPrice {
    price: String,
    expo: i32,
}

/// Snapshot of all five rates, all expressed as USD per 1 unit of the asset.
#[derive(Debug, Clone, Copy, Default)]
pub struct FxRates {
    /// USD per 1 SOL
    pub sol_usd: f64,
    /// USD per 1 EUR
    pub eur_usd: f64,
    /// USD per 1 GBP
    pub gbp_usd: f64,
    /// USD per 1 BRL
    pub brl_usd: f64,
    /// USD per 1 CAD
    pub cad_usd: f64,
}

impl FxRates {
    /// Convert a USD amount to the local currency for a given country code.
    pub fn usd_to_local(&self, usd: f64, country: &str) -> (f64, &'static str) {
        match country {
            "GB" => (usd / self.gbp_usd, "GBP"),
            "BR" => (usd / self.brl_usd, "BRL"),
            "CA" => (usd / self.cad_usd, "CAD"),
            "DE" | "AT" | "FR" | "NL" | "ES" | "IT" | "PT" => (usd / self.eur_usd, "EUR"),
            _ => (usd, "USD"),
        }
    }

    /// Currency symbol for display.
    pub fn symbol(currency: &str) -> &'static str {
        match currency {
            "GBP" => "£",
            "EUR" => "€",
            "BRL" => "R$",
            "CAD" => "C$",
            _ => "$",
        }
    }
}

/// Pyth oracle — fetches and caches live FX rates.
pub struct PythOracle {
    rates: RwLock<Option<FxRates>>,
}

impl PythOracle {
    pub fn new() -> Self {
        Self { rates: RwLock::new(None) }
    }

    /// Fetch all five rates in one Hermes batch call and cache them.
    pub async fn fetch_price(&self) -> Result<f64, String> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| format!("HTTP client: {e}"))?;

        let resp: HermesResponse = client
            .get(HERMES_URL)
            .send()
            .await
            .map_err(|e| format!("Hermes request failed: {e}"))?
            .json()
            .await
            .map_err(|e| format!("Hermes JSON parse: {e}"))?;

        if resp.parsed.len() < 5 {
            return Err(format!("Expected 5 price feeds, got {}", resp.parsed.len()));
        }

        let parse_feed = |idx: usize| -> Result<f64, String> {
            let p = &resp.parsed[idx];
            let mantissa: f64 = p.price.price.parse()
                .map_err(|e| format!("price[{idx}] parse: {e}"))?;
            Ok(mantissa * 10f64.powi(p.price.expo))
        };

        let rates = FxRates {
            sol_usd: parse_feed(SOL_IDX)?,
            eur_usd: parse_feed(EUR_IDX)?,
            gbp_usd: parse_feed(GBP_IDX)?,
            brl_usd: parse_feed(BRL_IDX)?,
            cad_usd: parse_feed(CAD_IDX)?,
        };

        *self.rates.write().unwrap() = Some(rates);
        Ok(rates.sol_usd)
    }

    /// All cached rates, or `None` if not yet fetched.
    pub fn get_rates(&self) -> Option<FxRates> {
        *self.rates.read().unwrap()
    }

    /// Cached SOL/USD only (backwards-compat helper).
    pub fn get_cached_price(&self) -> Option<f64> {
        self.get_rates().map(|r| r.sol_usd)
    }

    /// Convert lamports → USD using cached rate.
    pub fn lamports_to_usd(&self, lamports: u64) -> Option<f64> {
        let sol = lamports as f64 / 1_000_000_000.0;
        Some(sol * self.get_rates()?.sol_usd)
    }

    /// Convert GBP → lamports (backwards-compat).
    pub fn gbp_to_lamports(&self, gbp: f64) -> Option<u64> {
        let rates = self.get_rates()?;
        let usd = gbp * rates.gbp_usd;
        let sol = usd / rates.sol_usd;
        Some((sol * 1_000_000_000.0) as u64)
    }
}

impl Default for PythOracle {
    fn default() -> Self { Self::new() }
}
