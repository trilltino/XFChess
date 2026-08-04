//! Live but stubbed relayer-identity endpoints — mounted in `signing/mod.rs`
//! alongside the near-identical `solana::tee_relayer` routes. Both return
//! hardcoded placeholder strings, not real data; see `signing::tee_relayer`'s
//! module docs for the fuller context (that one is the more complete stub,
//! with a `TEERelayer` struct alongside its routes).

use crate::signing::AppState;
use axum::{extract::State, routing::get, Json, Router};
use serde_json::{json, Value};

/// Live at `GET /relayer/pubkey`. Always returns the hardcoded string
/// `"PlaceholderRelayerPubkey"`, not a real key.
async fn get_relayer_pubkey(State(_state): State<AppState>) -> Json<Value> {
    // Placeholder for actual relayer public key
    let relayer_pubkey = "PlaceholderRelayerPubkey";
    Json(json!({ "pubkey": relayer_pubkey }))
}

/// Live at `GET /relayer/attestation`. Always returns the hardcoded string
/// `"PlaceholderAttestationQuote"`, not a real attestation.
async fn get_relayer_attestation(State(_state): State<AppState>) -> Json<Value> {
    // Placeholder for actual attestation quote
    let attestation_quote = "PlaceholderAttestationQuote";
    Json(json!({ "quote": attestation_quote }))
}

/// Mounts the two placeholder relayer-identity endpoints. See module docs.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/relayer/pubkey", get(get_relayer_pubkey))
        .route("/relayer/attestation", get(get_relayer_attestation))
}
