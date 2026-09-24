use axum::{http::StatusCode, routing::post, Json, Router};
use serde::{Deserialize, Serialize};

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
    Router::new().route("/client-events", post(record_client_event))
}

async fn record_client_event(Json(event): Json<ClientEventReq>) -> StatusCode {
    let event_json = serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_string());
    tracing::info!(target: "client_events", "{}", event_json);
    StatusCode::ACCEPTED
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_event_schema_excludes_secret_fields() {
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
        let json = serde_json::to_string(&event).unwrap();
        assert!(!json.contains("session_signer_secret"));
        assert!(!json.contains("private"));
        assert!(!json.contains("keypair"));
    }
}
