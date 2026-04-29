//! KYC submission and user verification status endpoints.
//!
//! PII is stored in the vault SQLite database (separate from the session DB).
//! Tax IDs are stored only as SHA-256 blind hashes — raw values are never
//! persisted. Soft-delete and audit logging support GDPR right-to-erasure.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::signing::AppState;
use crate::signing::storage::vault::{KycInput, VaultStore};

/// Country-specific tax ID validation patterns.
fn get_tax_id_pattern(country: &str) -> Option<Regex> {
    match country {
        "GB" => Some(Regex::new(r"^[A-Za-z]{2}\d{6}[A-Za-z]$").unwrap()), // UK NI: AB123456C
        "BR" => Some(Regex::new(r"^\d{3}\.?\d{3}\.?\d{3}-?\d{2}$").unwrap()), // Brazil CPF
        "DE" => Some(Regex::new(r"^\d{11}$").unwrap()), // Germany Tax ID: 11 digits
        "CA" => Some(Regex::new(r"^\d{3}-?\d{3}-?\d{3}$").unwrap()), // Canada SIN
        _ => None,
    }
}

/// KYC submission payload from the frontend.
#[derive(Deserialize, Serialize, Clone)]
pub struct KycRequest {
    pub wallet_pubkey: String,
    pub country: String,
    pub full_name: String,
    pub dob: String,
    pub residence: String,
    pub tax_id: String,
}

#[derive(Serialize)]
pub struct OkResponse {
    pub ok: bool,
}

/// User verification status response.
#[derive(Serialize)]
pub struct UserStatus {
    pub has_profile: bool,
    pub has_email: bool,
    pub has_kyc: bool,
    pub can_wager: bool,
}

pub async fn submit_kyc(
    State(state): State<AppState>,
    Json(req): Json<KycRequest>,
) -> Result<Json<OkResponse>, StatusCode> {
    if req.wallet_pubkey.trim().is_empty()
        || req.full_name.trim().is_empty()
        || req.dob.trim().is_empty()
        || req.country.trim().is_empty()
    {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate tax ID format for supported countries
    if let Some(pattern) = get_tax_id_pattern(&req.country) {
        if !pattern.is_match(&req.tax_id) {
            tracing::warn!("[kyc] Invalid tax ID format for country {}: {}", req.country, req.tax_id);
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    let vault = VaultStore::new((*state.vault_pool).clone());
    vault
        .insert_kyc(KycInput {
            wallet_pubkey: &req.wallet_pubkey,
            country: &req.country,
            full_name: &req.full_name,
            dob: &req.dob,
            residence: &req.residence,
            tax_id_raw: &req.tax_id,
            data_source: "self_submitted",
        })
        .await
        .map_err(|e| {
            tracing::error!("[kyc] DB write failed for {}: {}", req.wallet_pubkey, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Update kyc_status in users table
    let _ = state
        .store
        .set_kyc_status(&req.wallet_pubkey, "pending")
        .await;

    Ok(Json(OkResponse { ok: true }))
}

pub async fn user_status(
    State(state): State<AppState>,
    Path(pubkey): Path<String>,
) -> Json<UserStatus> {
    if pubkey.trim().is_empty() {
        warn!("[kyc] user_status called with empty pubkey");
    }

    let vault = VaultStore::new((*state.vault_pool).clone());
    let has_kyc = vault.has_kyc(&pubkey).await;
    let has_email = state.store.find_user_by_wallet(&pubkey).await.is_some();

    // on-chain profile presence is confirmed by the frontend independently
    let has_profile = has_kyc || has_email;
    let can_wager = has_profile && has_email && has_kyc;

    Json(UserStatus {
        has_profile,
        has_email,
        has_kyc,
        can_wager,
    })
}

pub fn kyc_routes() -> Router<AppState> {
    Router::new()
        .route("/api/kyc/submit", post(submit_kyc))
        .route("/api/user/status/{pubkey}", get(user_status))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_router_builds() {
        // Just verify the route tree compiles
        let _r: Router<AppState> = kyc_routes();
    }
}
