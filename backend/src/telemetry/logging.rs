use std::fmt;
use std::time::Instant;
use uuid::Uuid;

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
            match wallet.char_indices().nth(8) {
                Some((end, _)) => write!(f, ",wallet={}...", &wallet[..end])?,
                None => write!(f, ",wallet={wallet}")?,
            }
        }
        if let Some(game_id) = self.game_id {
            write!(f, ",game={}", game_id)?;
        }
        write!(f, ",endpoint={}", self.endpoint)?;
        write!(f, "]")
    }
}

pub struct StructuredLogger;

impl StructuredLogger {
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

/// Stdout log writer that strips URLs, and the RPC provider tokens in their
/// paths, from every event. Upstream error text (reqwest, Solana client)
/// embeds the full request URL, and any `warn!("{e}")` would otherwise print it.
#[derive(Clone, Copy)]
pub struct RedactingStdout;

pub struct RedactingWriter;

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for RedactingStdout {
    type Writer = RedactingWriter;

    fn make_writer(&'a self) -> Self::Writer {
        RedactingWriter
    }
}

impl std::io::Write for RedactingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let text = String::from_utf8_lossy(buf);
        let scrubbed = crate::signing::solana::redact_urls_in(&text);
        std::io::stdout().lock().write_all(scrubbed.as_bytes())?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        std::io::Write::flush(&mut std::io::stdout())
    }
}

pub fn scrub_pii(input: &str) -> String {
    let mut result = input.to_string();

    // Scrub Solana pubkeys (base58, 32-44 chars)
    // This is a simple heuristic - may need refinement
    let pubkey_regex = match regex::Regex::new(r"[A-HJ-NP-Za-km-z1-9]{32,44}") {
        Ok(re) => re,
        Err(_) => return result, // Return original if regex fails
    };
    result = pubkey_regex.replace_all(&result, "<WALLET>").to_string();

    // Scrub emails
    let email_regex = match regex::Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}") {
        Ok(re) => re,
        Err(_) => return result, // Return original if regex fails
    };
    result = email_regex.replace_all(&result, "<EMAIL>").to_string();

    result
}

pub fn format_log_entry(
    level: &str,
    target: &str,
    message: &str,
    context: Option<&RequestContext>,
) -> String {
    let timestamp = chrono::Utc::now().to_rfc3339();

    match context {
        Some(ctx) => {
            format!("{} [{}] {} {} - {}", timestamp, level, target, ctx, message)
        }
        None => {
            format!("{} [{}] {} - {}", timestamp, level, target, message)
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
        let unicode = RequestContext::new("/api/test").with_wallet("♟♟♟♟♟♟♟♟♟");
        assert!(unicode.to_string().contains("wallet=♟♟♟♟♟♟♟♟..."));
    }

    #[test]
    fn test_pii_scrubbing() {
        let input =
            "User 7xKXtg2CW85dKr9YTCBz53b8fPFCJeGVsREjW2Qe5P logged in with email test@example.com";
        let scrubbed = scrub_pii(input);
        assert!(scrubbed.contains("<WALLET>"));
        assert!(scrubbed.contains("<EMAIL>"));
        assert!(!scrubbed.contains("7xKXtg2CW85dKr9YTCBz53b8fPFCJeGVsREjW2Qe5P"));
        assert!(!scrubbed.contains("test@example.com"));
    }
}
