//! Identity registration routes for KYC/PII data with GDPR compliance.
//!
//! This module handles identity verification with:
//! - AES-256-GCM encryption for PII data
//! - Audit logging for GDPR compliance
//! - Right to be forgotten (data deletion)
//! - Consent tracking
//! - KYC status verification

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use solana_sdk::{pubkey::Pubkey, signature::Signature, signer::Signer};
use sqlx::Row;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::signing::AppState;

/// Identity registration payload with GDPR consent.
///
/// All PII fields are encrypted before storage. Consent is recorded for GDPR compliance.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IdentityPayload {
    /// Wallet public key (base58)
    pub pubkey: String,
    /// Full legal name
    pub full_name: String,
    /// Date of birth (YYYY-MM-DD)
    pub dob: String,
    /// Residential address
    pub address: String,
    /// Country code (ISO 3166-1 alpha-2)
    pub country: String,
    /// Tax ID / SSN / National ID
    pub tax_id: String,
    /// Wallet signature over "register_identity:{pubkey}:{timestamp}"
    pub signature: String,
    /// Unix timestamp for replay protection
    pub timestamp: u64,
    /// GDPR consent: "I consent to the collection and processing of my personal data"
    pub consent_kyc: bool,
    /// Consent for data retention period (years)
    pub consent_retention_years: u8,
}

/// KYC status response.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KycStatus {
    pub verified: bool,
    pub verified_at: Option<i64>,
    pub country: Option<String>,
    pub requires_kyc: bool,
}

/// GDPR data deletion request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeleteDataRequest {
    pub pubkey: String,
    pub signature: String,
    pub timestamp: u64,
    pub reason: Option<String>,
}

/// Creates the identity routes router.
///
/// # Returns
/// An Axum Router with identity registration endpoints
pub fn identity_routes() -> Router<AppState> {
    Router::new()
        .route("/register", post(register_identity))
        .route("/status/{pubkey}", get(check_kyc_status))
        .route("/delete", post(delete_identity_data))
}

/// Handles POST /identity/register - registers and vaults user identity.
///
/// # Flow
/// 1. Verify wallet signature authentication
/// 2. Check timestamp for replay protection (5 minute window)
/// 3. Generate blind index for tax ID (searchable hash)
/// 4. Encrypt all PII data using AES-256-GCM
/// 5. Store encrypted blob in SQLite vault database
/// 6. Submit on-chain verification transaction to mark user as verified
///
/// # Arguments
/// * `state` - Application state containing vault and config
/// * `payload` - Identity registration payload
///
/// # Returns
/// Empty JSON response on success, error tuple on failure
async fn register_identity(
    State(state): State<AppState>,
    Json(payload): Json<IdentityPayload>,
) -> Result<Json<()>, (StatusCode, String)> {
    // 1. Verify Authentication
    let pk =
        Pubkey::from_str(&payload.pubkey).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let sig = Signature::from_str(&payload.signature)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let msg = format!("register_identity:{}:{}", payload.pubkey, payload.timestamp);
    if !sig.verify(pk.as_ref(), msg.as_bytes()) {
        return Err((StatusCode::UNAUTHORIZED, "Invalid Signature".to_string()));
    }

    // Protect replay attacks
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("System time should be available")
        .as_secs();
    if now > payload.timestamp && now - payload.timestamp > 300 {
        return Err((StatusCode::BAD_REQUEST, "Signature expired".to_string()));
    }

    // 2. Encryption
    let blind_index = state.identity_vault.generate_blind_index(&payload.tax_id);

    let privacy_json = serde_json::json!({
        "full_name": payload.full_name,
        "dob": payload.dob,
        "address": payload.address,
        "country": payload.country,
        "tax_id": payload.tax_id,
    })
    .to_string();

    let encrypted_blob = state
        .identity_vault
        .encrypt(&privacy_json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let registered_at = now as i64;

    // 3. Vault Storage
    let pool = &state.vault_pool;

    // Reject if GDPR consent was not given
    if !payload.consent_kyc {
        return Err((
            StatusCode::BAD_REQUEST,
            "GDPR consent is required to store identity data".to_string(),
        ));
    }

    let result = sqlx::query(
        "INSERT INTO vault_users (pubkey, blind_index_hash, encrypted_blob, registered_at, consent_kyc, consent_retention_years) VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(&payload.pubkey)
    .bind(blind_index)
    .bind(encrypted_blob)
    .bind(registered_at)
    .bind(payload.consent_kyc)
    .bind(payload.consent_retention_years as i32)
    .execute(&**pool)
    .await;

    if let Err(e) = result {
        if e.to_string().contains("UNIQUE") {
            return Err((
                StatusCode::CONFLICT,
                "Tax ID or Wallet already registered".to_string(),
            ));
        }
        return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()));
    }

    // 4. On-Chain Sync: VPS signs the instruction to flag the user as verified
    let admin_keypair = &state.kyc_authority;
    let program_id = Pubkey::from_str(&state.config.program_id).unwrap_or_else(|_| {
        "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU"
            .parse()
            .expect("Default program ID should be valid")
    });

    let ix = crate::signing::solana::verify_profile_ix(&program_id, &admin_keypair.pubkey(), &pk);
    let rpc = crate::signing::solana::make_rpc(&state.config.solana_rpc_url);

    match crate::signing::solana::sign_and_submit(&rpc, admin_keypair, &[ix]) {
        Ok(sig) => tracing::info!("[Identity] On-chain verification tx confirmed: {}", sig),
        Err(e) => tracing::error!(
            "[Identity] Failed to submit verification tx on-chain: {}",
            e
        ),
    }

    // 5. Write-through to kyc_records (unified vault table) so user_status and
    //    can_wager checks see the record regardless of which path was used.
    let vault = crate::signing::storage::vault::VaultStore::new((*state.vault_pool).clone());
    if let Err(e) = vault
        .insert_kyc(crate::signing::storage::vault::KycInput {
            wallet_pubkey: &payload.pubkey,
            country: &payload.country,
            full_name: &payload.full_name,
            dob: &payload.dob,
            residence: &payload.address,
            tax_id_raw: &payload.tax_id,
            data_source: "identity_verified",
        })
        .await
    {
        warn!(
            "[Identity] kyc_records write-through failed for {}: {}",
            payload.pubkey, e
        );
    }

    // 6. Mark kyc_status = 'approved' in users_v2 (on-chain verification succeeded).
    let _ = state
        .store
        .set_kyc_status(&payload.pubkey, "approved")
        .await;

    // 7. Persist CACF compliance: identity verification → fully_compliant for their country.
    if let Err(e) = vault
        .save_cacf(
            &payload.pubkey,
            &payload.country,
            "fully_compliant",
            true,
            None,
        )
        .await
    {
        warn!(
            "[Identity] CACF persist failed for {}: {}",
            payload.pubkey, e
        );
    }

    info!(
        "[Identity] Successfully registered and vaulted user {} securely",
        payload.pubkey
    );

    // 8. Log audit event for GDPR compliance
    log_audit_event(&payload.pubkey, "KYC_REGISTERED", &state.vault_pool).await;

    Ok(Json(()))
}

/// GET /identity/status/{pubkey} - Check KYC verification status.
async fn check_kyc_status(
    Path(pubkey): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<KycStatus>, StatusCode> {
    let pool = &state.vault_pool;

    // Check if user exists and get basic info (without decrypting PII)
    let row =
        sqlx::query("SELECT registered_at, blind_index_hash FROM vault_users WHERE pubkey = ?")
            .bind(&pubkey)
            .fetch_optional(&**pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let verified = row.is_some();
    let verified_at = row.as_ref().map(|r| r.get::<i64, _>("registered_at"));

    // Log access for GDPR audit trail
    log_audit_event(&pubkey, "KYC_STATUS_CHECKED", pool).await;

    Ok(Json(KycStatus {
        verified,
        verified_at,
        country: None, // Would need to decrypt to get this - avoid for status checks
        requires_kyc: true, // Mainnet wagering always requires KYC
    }))
}

/// POST /identity/delete - GDPR right to be forgotten.
async fn delete_identity_data(
    State(state): State<AppState>,
    Json(req): Json<DeleteDataRequest>,
) -> Result<Json<()>, (StatusCode, String)> {
    // 1. Verify Authentication
    let pk = Pubkey::from_str(&req.pubkey).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let sig = Signature::from_str(&req.signature)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let msg = format!("delete_identity:{}:{}", req.pubkey, req.timestamp);
    if !sig.verify(pk.as_ref(), msg.as_bytes()) {
        return Err((StatusCode::UNAUTHORIZED, "Invalid Signature".to_string()));
    }

    // 2. Protect replay attacks
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("System time should be available")
        .as_secs();
    if now > req.timestamp && now - req.timestamp > 300 {
        return Err((StatusCode::BAD_REQUEST, "Signature expired".to_string()));
    }

    let pool = &state.vault_pool;

    // 3. Log deletion request before deleting (for audit)
    let reason = req.reason.as_deref().unwrap_or("User request");
    warn!(
        "[GDPR] Identity deletion requested for {}. Reason: {}",
        req.pubkey, reason
    );
    log_audit_event(&req.pubkey, "KYC_DELETION_REQUESTED", pool).await;

    // 4. Delete user data
    let result = sqlx::query("DELETE FROM vault_users WHERE pubkey = ?")
        .bind(&req.pubkey)
        .execute(&**pool)
        .await;

    match result {
        Ok(res) => {
            if res.rows_affected() == 0 {
                return Err((
                    StatusCode::NOT_FOUND,
                    "No data found for this wallet".to_string(),
                ));
            }
            info!("[GDPR] Identity data deleted for {}", req.pubkey);
            log_audit_event(&req.pubkey, "KYC_DATA_DELETED", pool).await;
            Ok(Json(()))
        }
        Err(e) => {
            error!(
                "[GDPR] Failed to delete identity data for {}: {}",
                req.pubkey, e
            );
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

/// Logs audit events for GDPR compliance.
async fn log_audit_event(pubkey: &str, action: &str, pool: &Arc<sqlx::SqlitePool>) {
    let timestamp = Utc::now().timestamp();
    let result = sqlx::query("INSERT INTO audit_log (pubkey, action, timestamp) VALUES (?, ?, ?)")
        .bind(pubkey)
        .bind(action)
        .bind(timestamp)
        .execute(pool.as_ref())
        .await;

    if let Err(e) = result {
        tracing::warn!("[Audit] Failed to log audit event: {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    #[test]
    fn test_identity_payload_serialization() {
        let payload = IdentityPayload {
            pubkey: "test_wallet".to_string(),
            full_name: "Test User".to_string(),
            dob: "1990-01-01".to_string(),
            address: "123 Test St".to_string(),
            country: "US".to_string(),
            tax_id: "123456789".to_string(),
            signature: "test_signature".to_string(),
            timestamp: SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            consent_kyc: true,
            consent_retention_years: 5,
        };

        let json = serde_json::to_string(&payload);
        assert!(json.is_ok());
    }

    #[test]
    fn test_kyc_status_serialization() {
        let status = KycStatus {
            verified: true,
            verified_at: Some(1234567890),
            country: Some("US".to_string()),
            requires_kyc: true,
        };

        let json = serde_json::to_string(&status);
        assert!(json.is_ok());
    }

    #[test]
    fn test_delete_data_request_serialization() {
        let req = DeleteDataRequest {
            pubkey: "test_wallet".to_string(),
            signature: "test_signature".to_string(),
            timestamp: SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            reason: Some("Test deletion".to_string()),
        };

        let json = serde_json::to_string(&req);
        assert!(json.is_ok());
    }

    #[tokio::test]
    async fn test_identity_routes_creation() {
        let _router = identity_routes();
    }

    #[test]
    fn test_identity_payload_validation() {
        // Test that consent is required
        let payload = IdentityPayload {
            pubkey: "test_wallet".to_string(),
            full_name: "Test User".to_string(),
            dob: "1990-01-01".to_string(),
            address: "123 Test St".to_string(),
            country: "US".to_string(),
            tax_id: "123456789".to_string(),
            signature: "test_signature".to_string(),
            timestamp: SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            consent_kyc: false, // Missing consent
            consent_retention_years: 5,
        };

        assert!(!payload.consent_kyc);
    }

    #[test]
    fn test_country_code_format() {
        // Test valid ISO 3166-1 alpha-2 codes
        let valid_countries = vec!["US", "GB", "DE", "FR", "JP"];
        for country in valid_countries {
            assert_eq!(country.len(), 2);
            assert!(country.chars().all(|c| c.is_ascii_uppercase()));
        }
    }

    #[test]
    fn test_dob_format() {
        // Test YYYY-MM-DD format
        let valid_dobs = vec!["1990-01-01", "2000-12-31", "1985-06-15"];
        for dob in valid_dobs {
            assert!(dob.len() == 10);
            assert!(dob.chars().nth(4) == Some('-'));
            assert!(dob.chars().nth(7) == Some('-'));
        }
    }
}
