//! Blinks funding integration helpers for MoonPay/Transak/Banxa.
//!
//! Stub: API keys and SDK integration deferred until production deployment.

/// Build a MoonPay widget URL for the given wallet.
pub fn moonpay_url(wallet: &str, amount_usd: f64) -> String {
    // TODO: insert real API key from env
    format!(
        "https://buy.moonpay.com?walletAddress={}&currencyCode=SOL&baseCurrencyAmount={}",
        wallet, amount_usd
    )
}

/// Build a Transak widget URL for the given wallet.
pub fn transak_url(wallet: &str, amount_usd: f64) -> String {
    format!(
        "https://global.transak.com?walletAddress={}&fiatAmount={}&fiatCurrency=USD&cryptoCurrencyCode=SOL",
        wallet, amount_usd
    )
}
