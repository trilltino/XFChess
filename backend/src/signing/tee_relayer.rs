//! Stub for a planned MagicBlock TEE (Trusted Execution Environment) relayer
//! integration — not implemented. `TEERelayer::sign_and_submit`,
//! `get_public_key`, and `get_attestation_quote` are unused by the rest of
//! the codebase; real transaction signing goes through
//! `signing::solana::sign_and_submit` everywhere instead.
//!
//! `routes()` below **is** mounted live (`signing/mod.rs`): `GET /tee/pubkey`
//! and `GET /tee/attestation` currently return the hardcoded strings
//! `"PlaceholderTEEPubkey"` / `"PlaceholderTEEAttestation"` to any caller,
//! not real data. Anything consuming these two endpoints today is reading
//! placeholders, not a TEE attestation.

use crate::error::AppError;
use crate::signing::AppState;
use axum::{extract::State, routing::get, Json, Router};
use serde_json::{json, Value};
use solana_sdk::instruction::Instruction;
use solana_sdk::pubkey::Pubkey;

/// Unused stub for a planned MagicBlock TEE relayer client. See module docs.
pub struct TEERelayer;

impl TEERelayer {
    /// Creates a new TEERelayer instance
    pub fn new() -> Result<Self, AppError> {
        Ok(TEERelayer)
    }

    /// Not implemented — always returns a fixed placeholder string, never
    /// actually signs or submits anything. Not called anywhere in the codebase.
    pub async fn sign_and_submit(
        &self,
        _instructions: Vec<Instruction>,
        _recent_blockhash: &str,
    ) -> Result<String, AppError> {
        // Placeholder for actual HTTP request to TEE endpoint
        // Build unsigned transaction with TEE pubkey as fee_payer
        // POST to TEE for signing
        // Forward signed transaction to Solana/ER RPC
        // Return signature
        Ok("placeholder_signature".to_string())
    }

    /// Not implemented — always errors. Not called anywhere in the codebase.
    pub async fn get_public_key(&self) -> Result<Pubkey, AppError> {
        // TODO: Fetch actual TEE relayer public key from attestation service
        // For now, return an error since default pubkey is meaningless
        Err(AppError::Internal(
            "TEE relayer public key not yet implemented".to_string(),
        ))
    }

    /// Not implemented — always returns a fixed placeholder string. Not
    /// called anywhere in the codebase.
    pub async fn get_attestation_quote(&self) -> Result<String, AppError> {
        // Placeholder for actual attestation quote retrieval
        Ok("placeholder_attestation_quote".to_string())
    }
}

/// Live HTTP handler for `GET /tee/pubkey` — returns the hardcoded string
/// `"PlaceholderTEEPubkey"`, not a real key. See module docs.
async fn get_tee_pubkey(State(_state): State<AppState>) -> Json<Value> {
    // Placeholder for actual TEE public key
    let tee_pubkey = "PlaceholderTEEPubkey";
    Json(json!({ "pubkey": tee_pubkey }))
}

/// Live HTTP handler for `GET /tee/attestation` — returns the hardcoded
/// string `"PlaceholderTEEAttestation"`, not a real attestation. See module docs.
async fn get_tee_attestation(State(_state): State<AppState>) -> Json<Value> {
    // Placeholder for actual attestation quote
    let attestation = "PlaceholderTEEAttestation";
    Json(json!({ "attestation": attestation }))
}

/// Mounts the two placeholder TEE informational endpoints. See module docs —
/// these return fixed strings, not real TEE data.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/tee/pubkey", get(get_tee_pubkey))
        .route("/tee/attestation", get(get_tee_attestation))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tee_relayer_new() {
        let relayer = TEERelayer::new();
        assert!(relayer.is_ok());
    }
}
