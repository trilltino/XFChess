use serde::Serialize;
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Serialize)]
pub struct ClientEvent {
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet_pubkey: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub client_version: String,
    pub platform: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend_region: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tournament_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    pub timestamp_ms: i64,
}

impl ClientEvent {
    pub fn new(event: impl Into<String>) -> Self {
        Self {
            event: event.into(),
            wallet_pubkey: None,
            game_id: None,
            role: None,
            session_kind: None,
            reason: None,
            client_version: env!("CARGO_PKG_VERSION").to_string(),
            platform: std::env::consts::OS.to_string(),
            backend_region: std::env::var("XFCHESS_BACKEND_REGION").ok(),
            tournament_id: None,
            action: None,
            signature: None,
            status: None,
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn wallet(mut self, wallet: impl ToString) -> Self {
        self.wallet_pubkey = Some(wallet.to_string());
        self
    }

    pub fn game_id(mut self, game_id: impl ToString) -> Self {
        self.game_id = Some(game_id.to_string());
        self
    }

    pub fn role(mut self, role: impl ToString) -> Self {
        self.role = Some(role.to_string());
        self
    }

    pub fn session_kind(mut self, kind: impl Into<String>) -> Self {
        self.session_kind = Some(kind.into());
        self
    }

    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(redact(reason.into()));
        self
    }

    pub fn tournament_id(mut self, tournament_id: impl ToString) -> Self {
        self.tournament_id = Some(tournament_id.to_string());
        self
    }

    pub fn action(mut self, action: impl Into<String>) -> Self {
        self.action = Some(action.into());
        self
    }

    pub fn signature(mut self, signature: impl ToString) -> Self {
        self.signature = Some(signature.to_string());
        self
    }

    pub fn status(mut self, status: impl Into<String>) -> Self {
        self.status = Some(status.into());
        self
    }

    fn dedupe_key(&self) -> String {
        format!(
            "{}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
            self.event,
            self.wallet_pubkey,
            self.game_id,
            self.session_kind,
            self.reason,
            self.tournament_id,
            self.action,
            self.status
        )
    }
}

pub fn emit_client_event(event: ClientEvent) {
    if was_seen(&event) {
        return;
    }
    std::thread::spawn(move || {
        let client = match super::client_fast() {
            Ok(client) => client,
            Err(e) => {
                tracing::debug!("[client-events] skipped: {e}");
                return;
            }
        };
        let resp = client
            .post(format!("{}/api/client-events", super::vps_base()))
            .json(&event)
            .send();
        match resp {
            Ok(resp) if resp.status().is_success() => {}
            Ok(resp) => tracing::debug!("[client-events] backend returned {}", resp.status()),
            Err(e) => tracing::debug!("[client-events] send failed: {e}"),
        }
    });
}

fn was_seen(event: &ClientEvent) -> bool {
    static SEEN: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let seen = SEEN.get_or_init(|| Mutex::new(HashSet::new()));
    let Ok(mut seen) = seen.lock() else {
        return false;
    };
    !seen.insert(event.dedupe_key())
}

fn redact(mut value: String) -> String {
    const MAX_REASON_LEN: usize = 240;
    for marker in [
        "secret",
        "private",
        "seed",
        "keypair",
        "session_signer_secret",
    ] {
        if value.to_ascii_lowercase().contains(marker) {
            value = "redacted".to_string();
            break;
        }
    }
    value.truncate(MAX_REASON_LEN);
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reason_redacts_secret_like_words() {
        let event = ClientEvent::new("test").reason("session_signer_secret leaked");
        assert_eq!(event.reason.as_deref(), Some("redacted"));
    }

    #[test]
    fn serialized_event_has_no_secret_material_fields() {
        let event = ClientEvent::new("test")
            .wallet("wallet")
            .session_kind("standalone")
            .reason("safe public reason");
        let json = serde_json::to_string(&event).unwrap();
        assert!(!json.contains("session_signer_secret"));
        assert!(!json.contains("keypair"));
        assert!(!json.contains("private"));
        assert!(json.contains("safe public reason"));
    }
}
