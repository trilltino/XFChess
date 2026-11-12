use axum::{http::StatusCode, routing::post, Json, Router};
use serde::{Deserialize, Serialize};

use crate::signing::node_auth::{attach_node_identity, RequireNode};
use crate::signing::AppState;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientEventReq {
    pub event: String,
    pub wallet_pubkey: Option<String>,
    pub game_id: Option<String>,
    pub role: Option<String>,
    pub session_kind: Option<String>,
    pub reason: Option<String>,
    pub client_version: Option<String>,
    pub platform: Option<String>,
    pub backend_region: Option<String>,
    pub tournament_id: Option<String>,
    pub action: Option<String>,
    pub signature: Option<String>,
    pub status: Option<String>,
    pub timestamp_ms: Option<i64>,
}

pub fn client_events_routes() -> Router<AppState> {
    Router::new()
        .route("/client-events", post(record_client_event))
        .layer(axum::middleware::from_fn(attach_node_identity))
}

/// Longest accepted field; an event is a log line, not a document.
const MAX_FIELD_LEN: usize = 256;

fn has_oversized_field(event: &ClientEventReq) -> bool {
    [
        Some(&event.event),
        event.wallet_pubkey.as_ref(),
        event.game_id.as_ref(),
        event.role.as_ref(),
        event.session_kind.as_ref(),
        event.reason.as_ref(),
        event.client_version.as_ref(),
        event.platform.as_ref(),
        event.backend_region.as_ref(),
        event.tournament_id.as_ref(),
        event.action.as_ref(),
        event.signature.as_ref(),
        event.status.as_ref(),
    ]
    .into_iter()
    .flatten()
    .any(|field| field.len() > MAX_FIELD_LEN)
}

/// Events are attributed to the node that sent them, so a caller cannot write log lines as
/// someone else; `wallet_pubkey` is the caller's own claim and is logged as such.
async fn record_client_event(node: RequireNode, Json(event): Json<ClientEventReq>) -> StatusCode {
    if has_oversized_field(&event) {
        return StatusCode::UNPROCESSABLE_ENTITY;
    }
    let event_json = serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_string());
    tracing::info!(target: "client_events", node_id = %node.0, "{}", event_json);
    StatusCode::ACCEPTED
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context as _;

    #[test]
    fn client_event_schema_excludes_secret_fields() -> anyhow::Result<()> {
        let event = ClientEventReq {
            event: "solana_session_expired".to_string(),
            wallet_pubkey: Some("wallet".to_string()),
            game_id: Some("1".to_string()),
            role: Some("host".to_string()),
            session_kind: Some("expired".to_string()),
            reason: Some("expired".to_string()),
            client_version: Some("0.1.0".to_string()),
            platform: Some("windows".to_string()),
            backend_region: Some("eu".to_string()),
            tournament_id: Some("9".to_string()),
            action: Some("leave_tournament".to_string()),
            signature: Some("sig".to_string()),
            status: Some("confirmed".to_string()),
            timestamp_ms: Some(1),
        };
        let json = serde_json::to_string(&event).context("test setup failed")?;
        assert!(!json.contains("session_signer_secret"));
        assert!(!json.contains("private"));
        assert!(!json.contains("keypair"));
        Ok(())
    }
}
