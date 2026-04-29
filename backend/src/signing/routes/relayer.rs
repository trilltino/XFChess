use axum::{
    routing::get,
    Router, Json, extract::State,
};
use serde_json::{json, Value};
use crate::signing::AppState;

/// Returns the relayer public key and attestation for client verification
async fn get_relayer_pubkey(
    State(_state): State<AppState>,
) -> Json<Value> {
    // Placeholder for actual relayer public key
    let relayer_pubkey = "PlaceholderRelayerPubkey";
    Json(json!({ "pubkey": relayer_pubkey }))
}

/// Returns the current TEE attestation quote for verification
async fn get_relayer_attestation(
    State(_state): State<AppState>,
) -> Json<Value> {
    // Placeholder for actual attestation quote
    let attestation_quote = "PlaceholderAttestationQuote";
    Json(json!({ "quote": attestation_quote }))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/relayer/pubkey", get(get_relayer_pubkey))
        .route("/relayer/attestation", get(get_relayer_attestation))
}
