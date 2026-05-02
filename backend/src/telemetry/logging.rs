//! Structured logging with request tracing for XFChess backend
//!
//! Provides correlation IDs for tracing requests across async boundaries
//! and structured log output for easier parsing and filtering.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use uuid::Uuid;

/// Context for a single request
#[derive(Clone, Debug)]
pub struct RequestContext {
    pub request_id: Uuid,
    pub wallet_pubkey: Option<String>,
    pub game_id: Option<u64>,
    pub endpoint: String,
    pub start_time: Instant,
}

impl RequestContext {
    pub fn new(endpoint: &str) -> Self {
        Self {
            request_id: Uuid::new_v4(),
            wallet_pubkey: None,
            game_id: None,
            endpoint: endpoint.to_string(),
            start_time: Instant::now(),
        }
    }
    
    pub fn with_wallet(mut self, pubkey: impl Into<String>) -> Self {
        self.wallet_pubkey = Some(pubkey.into());
        self
    }
    
    pub fn with_game_id(mut self, game_id: u64) -> Self {
        self.game_id = Some(game_id);
        self
    }
    
    pub fn elapsed_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }
}

impl fmt::Display for RequestContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[req={}", self.request_id)?;
        if let Some(wallet) = &self.wallet_pubkey {
            // Truncate wallet for privacy in logs
            let truncated = if wallet.len() > 8 {
                format!("{}...", &wallet[..8])
            } else {
                wallet.clone()
            };
            write!(f, ",wallet={}", truncated)?;
        }
        if let Some(game_id) = self.game_id {
            write!(f, ",game={}", game_id)?;
        }
        write!(f, ",endpoint={}", self.endpoint)?;
        write!(f, "]")
    }
}

/// Structured logger that includes request context
pub struct StructuredLogger;

impl StructuredLogger {
    /// Log info with context
    pub fn info(ctx: &RequestContext, message: &str) {
        tracing::info!(
            request_id = %ctx.request_id,
            endpoint = %ctx.endpoint,
            wallet = ctx.wallet_pubkey.as_deref().unwrap_or("none"),
            game_id = ctx.game_id.unwrap_or(0),
            duration_ms = ctx.elapsed_ms(),
            "{}",
            message
        );
    }
    
    /// Log error with context
    pub fn error(ctx: &RequestContext, message: &str, error: &dyn std::error::Error) {
        tracing::error!(
            request_id = %ctx.request_id,
            endpoint = %ctx.endpoint,
            wallet = ctx.wallet_pubkey.as_deref().unwrap_or("none"),
            game_id = ctx.game_id.unwrap_or(0),
            duration_ms = ctx.elapsed_ms(),
            error = %error,
            "{}",
            message
        );
    }
    
    /// Log warning with context
    pub fn warn(ctx: &RequestContext, message: &str) {
        tracing::warn!(
            request_id = %ctx.request_id,
            endpoint = %ctx.endpoint,
            wallet = ctx.wallet_pubkey.as_deref().unwrap_or("none"),
            game_id = ctx.game_id.unwrap_or(0),
            duration_ms = ctx.elapsed_ms(),
            "{}",
            message
        );
    }
    
    /// Log transaction event
    pub fn transaction(
        ctx: &RequestContext,
        signature: &str,
        status: &str,
        chain: &str,
        confirmation_time_ms: Option<u64>,
    ) {
        match confirmation_time_ms {
            Some(time) => {
                tracing::info!(
                    request_id = %ctx.request_id,
                    signature = signature,
                    status = status,
                    chain = chain,
                    confirmation_time_ms = time,
                    game_id = ctx.game_id.unwrap_or(0),
                    "transaction_confirmed"
                );
            }
            None => {
                tracing::info!(
                    request_id = %ctx.request_id,
                    signature = signature,
                    status = status,
                    chain = chain,
                    game_id = ctx.game_id.unwrap_or(0),
                    "transaction_event"
                );
            }
        }
    }
}

/// Scrub PII from log messages
pub fn scrub_pii(input: &str) -> String {
    let mut result = input.to_string();
    
    // Scrub Solana pubkeys (base58, 32-44 chars)
    // This is a simple heuristic - may need refinement
    let pubkey_regex = regex::Regex::new(r"[A-HJ-NP-Za-km-z1-9]{32,44}").unwrap();
    result = pubkey_regex.replace_all(&result, "<WALLET>").to_string();
    
    // Scrub emails
    let email_regex = regex::Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap();
    result = email_regex.replace_all(&result, "<EMAIL>").to_string();
    
    result
}

/// Log formatter that adds structured fields
pub fn format_log_entry(
    level: &str,
    target: &str,
    message: &str,
    context: Option<&RequestContext>,
) -> String {
    let timestamp = chrono::Utc::now().to_rfc3339();
    
    match context {
        Some(ctx) => {
            format!(
                "{} [{}] {} {} - {}",
                timestamp, level, target, ctx, message
            )
        }
        None => {
            format!(
                "{} [{}] {} - {}",
                timestamp, level, target, message
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_request_context_display() {
        let ctx = RequestContext::new("/api/test")
            .with_wallet("7xKXtg2CW85dKr9YTCB")
            .with_game_id(12345);
        
        let display = format!("{}", ctx);
        assert!(display.contains("endpoint=/api/test"));
        assert!(display.contains("game=12345"));
        assert!(display.contains("wallet=7xKXtg2C..."));
    }
    
    #[test]
    fn test_pii_scrubbing() {
        let input = "User 7xKXtg2CW85dKr9YTCBz53b8fPFCJeGVsREjW2Qe5P logged in with email test@example.com";
        let scrubbed = scrub_pii(input);
        assert!(scrubbed.contains("<WALLET>"));
        assert!(scrubbed.contains("<EMAIL>"));
        assert!(!scrubbed.contains("7xKXtg2CW85dKr9YTCBz53b8fPFCJeGVsREjW2Qe5P"));
        assert!(!scrubbed.contains("test@example.com"));
    }
}
