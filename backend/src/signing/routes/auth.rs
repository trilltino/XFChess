//! Wallet-first authentication for XFChess.
//!
//! The Solana wallet IS the identity — no passwords required.
//! All endpoints verify a cryptographic signature over `"xfchess:<action>:<timestamp>"`
//! to prove wallet ownership before issuing a JWT.
//!
//! # Endpoints
//! - `POST /auth/register`           — Create account (wallet + username + optional email)
//! - `POST /auth/login`              — Login with wallet signature → JWT
//! - `GET  /auth/check-username/:u`  — Check username availability
//! - `POST /auth/delete`             — GDPR right-to-erasure (wallet signature required)

use axum::{
    extract::State,
    http::StatusCode,
    Json, Router,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tracing::{info, error};
use crate::error::{AppError, AppResult};
use crate::signing::AppState;
use solana_sdk::{pubkey::Pubkey, signature::Signature};
use std::str::FromStr;

/// Verifies a wallet signature over `"xfchess:<action>:<timestamp>"`.
/// Returns `Err` with an appropriate HTTP status on failure.
fn verify_wallet_sig(
    wallet: &str,
    signature: &str,
    action: &str,
    timestamp: u64,
) -> Result<Pubkey, (StatusCode, String)> {
    let pk = Pubkey::from_str(wallet)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid wallet address".to_string()))?;
    let sig = Signature::from_str(signature)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid signature format".to_string()))?;
    let msg = format!("xfchess:{}:{}", action, timestamp);
    if !sig.verify(pk.as_ref(), msg.as_bytes()) {
        return Err((StatusCode::UNAUTHORIZED, "Signature verification failed".to_string()));
    }
    Ok(pk)
}

/// Creates the authentication router.
pub fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/check-username/:username", get(check_username))
        .route("/delete", post(delete_account))
}

// ── Shared response ────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct AuthResp {
    pub token: String,
    pub username: String,
    pub wallet: String,
}

// ── Register ───────────────────────────────────────────────────────────────────

/// POST /auth/register — Create a new account.
/// Body: `{ wallet, signature, timestamp, username, email? }`
/// The signature must cover `"xfchess:register:<timestamp>"`.
#[derive(Deserialize)]
struct RegisterReq {
    wallet: String,
    signature: String,
    timestamp: u64,
    username: String,
    email: Option<String>,
}

async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterReq>,
) -> Result<Json<AuthResp>, (StatusCode, String)> {
    verify_wallet_sig(&req.wallet, &req.signature, "register", req.timestamp)?;

    if state.store.find_user_by_wallet(&req.wallet).await.is_some() {
        return Err((StatusCode::CONFLICT, "Wallet already registered".to_string()));
    }
    if state.store.username_taken(&req.username).await {
        return Err((StatusCode::CONFLICT, "Username already taken".to_string()));
    }

    state
        .store
        .create_wallet_user(&req.wallet, &req.username, req.email.as_deref())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let token = state.jwt.issue(&req.wallet)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    info!("[Auth] Registered wallet: {}", req.wallet);
    Ok(Json(AuthResp { token, username: req.username, wallet: req.wallet }))
}

// ── Login ──────────────────────────────────────────────────────────────────────

/// POST /auth/login — Authenticate with wallet signature → JWT.
/// Body: `{ wallet, signature, timestamp }`
/// The signature must cover `"xfchess:login:<timestamp>"`.
#[derive(Deserialize)]
pub struct LoginReq {
    pub wallet: String,
    pub signature: String,
    pub timestamp: u64,
}

async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginReq>,
) -> Result<Json<AuthResp>, (StatusCode, String)> {
    verify_wallet_sig(&req.wallet, &req.signature, "login", req.timestamp)?;

    let user = state.store.find_user_by_wallet(&req.wallet).await
        .ok_or((StatusCode::UNAUTHORIZED, "Wallet not registered. Please create an account first.".to_string()))?;

    let token = state.jwt.issue(&req.wallet)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    info!("[Auth] Login: {}", req.wallet);
    Ok(Json(AuthResp { token, username: user.1, wallet: req.wallet }))
}

// ── Check username ─────────────────────────────────────────────────────────────

async fn check_username(
    State(state): State<AppState>,
    axum::extract::Path(username): axum::extract::Path<String>,
) -> Json<serde_json::Value> {
    let taken = state.store.username_taken(&username).await;
    Json(serde_json::json!({ "taken": taken }))
}

// ── GDPR delete ────────────────────────────────────────────────────────────────

/// POST /auth/delete — GDPR right-to-erasure.
/// Body: `{ wallet, signature, timestamp, reason? }`
/// The signature must cover `"xfchess:delete:<timestamp>"`.
#[derive(Deserialize)]
struct DeleteReq {
    wallet: String,
    signature: String,
    timestamp: u64,
    reason: Option<String>,
}

async fn delete_account(
    State(state): State<AppState>,
    Json(req): Json<DeleteReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    verify_wallet_sig(&req.wallet, &req.signature, "delete", req.timestamp)?;

    state.store.find_user_by_wallet(&req.wallet).await
        .ok_or((StatusCode::NOT_FOUND, "Wallet not registered".to_string()))?;

    // 1. Erase auth record
    state.store.erase_user(&req.wallet).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 2. Erase KYC PII from vault and write audit trail
    let vault = crate::signing::storage::vault::VaultStore::new((*state.vault_pool).clone());
    let _ = vault.erase_kyc(&req.wallet).await;
    let _ = vault.log_deletion_request(&req.wallet, None, req.reason.as_deref()).await;
    let _ = vault.complete_deletion_request(&req.wallet).await;
    vault.write_audit(&req.wallet, "account_deleted").await;

    info!("[Auth] GDPR erasure: {}", req.wallet);
    Ok(Json(serde_json::json!({ "ok": true, "message": "Account and KYC data erased." })))
}

// ── JWT issue (internal) ───────────────────────────────────────────────────────

/// POST /auth/issue — Issues a JWT (used internally by session-key flow).
pub async fn issue_jwt(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    let wallet = body
        .get("pubkey")
        .or_else(|| body.get("wallet_pubkey"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Missing pubkey".to_string()))?;
    let token = state.jwt.issue(wallet).map_err(|e| {
        error!("JWT issue error: {e}");
        AppError::Internal("Failed to issue token".to_string())
    })?;
    Ok(Json(serde_json::json!({ "token": token })))
}
