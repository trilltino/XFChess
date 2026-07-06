use crate::signing::AppState;
use axum::{extract::State, routing::get, Json, Router};
use serde_json::{json, Value};

/// Returns the relayer public key and attestation for client verification
async fn get_relayer_pubkey(State(_state): State<AppState>) -> Json<Value> {
    // Placeholder for actual relayer public key
    let relayer_pubkey = "PlaceholderRelayerPubkey";
    Json(json!({ "pubkey": relayer_pubkey }))
}

/// Returns the current TEE attestation quote for verification
async fn get_relayer_attestation(State(_state): State<AppState>) -> Json<Value> {
    // Placeholder for actual attestation quote
    let attestation_quote = "PlaceholderAttestationQuote";
    Json(json!({ "quote": attestation_quote }))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/relayer/pubkey", get(get_relayer_pubkey))
        .route("/relayer/attestation", get(get_relayer_attestation))
}
