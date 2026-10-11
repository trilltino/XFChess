//! Linking a Lichess account (ratings and puzzle training).

use super::{client_fast, vps_base};

/// Starts the Lichess OAuth flow for `wallet` in the system browser. Runs the
/// backend request off-thread; the backend binds the flow to the wallet's JWT.
pub fn start_lichess_link(wallet: String) {
    crate::core::net_task::spawn_detached("lichess-link", move || {
        let client = match client_fast() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("[Lichess] failed to build HTTP client: {e}");
                return;
            }
        };
        // Base58 pubkeys are alphanumeric only — no percent-encoding needed.
        let url = format!(
            "{}/api/auth/lichess/init?wallet_pubkey={wallet}",
            vps_base()
        );
        match client
            .get(url)
            .send()
            .and_then(|r| r.json::<serde_json::Value>())
        {
            Ok(body) => match body["auth_url"].as_str() {
                Some(auth_url) => {
                    let _ = webbrowser::open(auth_url);
                }
                None => tracing::warn!("[Lichess] /init response missing auth_url: {body}"),
            },
            Err(e) => tracing::warn!("[Lichess] /init request failed: {e}"),
        }
    });
}
