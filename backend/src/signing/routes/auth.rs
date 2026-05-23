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
use borsh::BorshDeserialize;
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
        .route("/register-email", post(register_email))
        .route("/login", post(login))
        .route("/login-email", post(login_email))
        .route("/link-wallet", post(link_wallet))
        .route("/me", get(me))
        .route("/add-email", post(add_email))
        .route("/sync-profile", post(sync_profile))
        .route("/username", axum::routing::patch(set_username))
        .route("/check-username/{username}", get(check_username))
        .route("/check-wallet/{wallet}", get(check_wallet))
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

// ── Email/Password Auth ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct RegisterEmailReq {
    email: String,
    password: String,
    username: String,
}

async fn register_email(
    State(state): State<AppState>,
    Json(req): Json<RegisterEmailReq>,
) -> Result<Json<AuthResp>, (StatusCode, String)> {
    if state.store.find_user_by_email(&req.email).await.is_some() {
        return Err((StatusCode::CONFLICT, "Email already registered".to_string()));
    }
    if state.store.username_taken(&req.username).await {
        return Err((StatusCode::CONFLICT, "Username already taken".to_string()));
    }

    use argon2::{
        password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
        Argon2,
    };
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(req.password.as_bytes(), &salt)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .to_string();

    state
        .store
        .register_with_email(&req.email, &req.username, &password_hash)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let token = state.jwt.issue(&format!("email:{}", req.email))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    info!("[Auth] Registered email user: {}", req.email);
    Ok(Json(AuthResp { token, username: req.username, wallet: "".to_string() }))
}

#[derive(Deserialize)]
struct LoginEmailReq {
    email: String,
    password: String,
}

async fn login_email(
    State(state): State<AppState>,
    Json(req): Json<LoginEmailReq>,
) -> Result<Json<AuthResp>, (StatusCode, String)> {
    let user = state.store.find_user_by_email(&req.email).await
        .ok_or((StatusCode::UNAUTHORIZED, "Invalid email or password".to_string()))?;

    let stored_hash = user.4.ok_or((StatusCode::UNAUTHORIZED, "This account does not have a password. Please login with wallet.".to_string()))?;

    use argon2::{password_hash::PasswordHash, password_hash::PasswordVerifier, Argon2};
    let parsed_hash = PasswordHash::new(&stored_hash)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Argon2::default()
        .verify_password(req.password.as_bytes(), &parsed_hash)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid email or password".to_string()))?;

    let identity = if !user.0.is_empty() {
        user.0.clone()
    } else {
        format!("email:{}", req.email)
    };

    let token = state.jwt.issue(&identity)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    info!("[Auth] Login email: {}", req.email);
    Ok(Json(AuthResp { token, username: user.1, wallet: user.0 }))
}

#[derive(Deserialize)]
struct LinkWalletReq {
    email: String,
    password: String,
    wallet: String,
    signature: String,
    timestamp: u64,
}

async fn link_wallet(
    State(state): State<AppState>,
    Json(req): Json<LinkWalletReq>,
) -> Result<Json<()>, (StatusCode, String)> {
    // 1. Verify Wallet Signature
    verify_wallet_sig(&req.wallet, &req.signature, "link", req.timestamp)?;

    // 2. Verify Email/Password
    let user = state.store.find_user_by_email(&req.email).await
        .ok_or((StatusCode::UNAUTHORIZED, "Invalid email or password".to_string()))?;

    let stored_hash = user.4.ok_or((StatusCode::BAD_REQUEST, "Account has no password".to_string()))?;
    use argon2::{password_hash::PasswordHash, password_hash::PasswordVerifier, Argon2};
    let parsed_hash = PasswordHash::new(&stored_hash)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Argon2::default()
        .verify_password(req.password.as_bytes(), &parsed_hash)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid email or password".to_string()))?;

    // 3. Link Wallet
    state.store.link_wallet(&req.email, &req.wallet).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    info!("[Auth] Linked wallet {} to email {}", req.wallet, req.email);
    Ok(Json(()))
}

// ── GET /auth/me ───────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct MeResp {
    wallet: String,
    username: String,
    email: Option<String>,
    kyc_status: String,
    /// True when a real Solana wallet pubkey is linked (not an email-only account).
    wallet_linked: bool,
    /// True when the account has a linked wallet, an approved KYC record in the
    /// vault, and CACF compliance for their jurisdiction.
    can_wager: bool,
}

/// GET /auth/me — validates Bearer JWT and returns caller profile.
async fn me(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<MeResp>, (StatusCode, String)> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(crate::signing::auth::extract_bearer)
        .ok_or((StatusCode::UNAUTHORIZED, "Missing Authorization header".to_string()))?;

    let claims = state.jwt.verify(token)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid or expired token".to_string()))?;

    let user = state.store.find_user_by_wallet(&claims.sub).await
        .ok_or((StatusCode::UNAUTHORIZED, "Account not found".to_string()))?;

    let wallet_linked = !user.0.is_empty();

    // Compute can_wager: wallet linked + vault KYC record + CACF ok.
    let vault = crate::signing::storage::vault::VaultStore::new((*state.vault_pool).clone());
    let has_kyc = vault.has_kyc(&user.0).await;
    let kyc_country = vault.get_kyc(&user.0).await.map(|r| r.country);
    let cacf_ok = match &kyc_country {
        Some(c) => vault.cacf_can_wager(&user.0, c).await,
        None => true,
    };
    let can_wager = wallet_linked && has_kyc && cacf_ok;

    Ok(Json(MeResp {
        wallet: user.0,
        username: user.1,
        email: user.2,
        kyc_status: user.3,
        wallet_linked,
        can_wager,
    }))
}

// ── POST /auth/add-email ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct AddEmailReq {
    email: String,
}

/// POST /auth/add-email — attaches an email to an existing wallet account.
/// Requires a valid Bearer JWT.
async fn add_email(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<AddEmailReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(crate::signing::auth::extract_bearer)
        .ok_or((StatusCode::UNAUTHORIZED, "Missing Authorization header".to_string()))?;

    let claims = state.jwt.verify(token)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid or expired token".to_string()))?;
    let wallet = claims.sub;

    state.store.find_user_by_wallet(&wallet).await
        .ok_or((StatusCode::UNAUTHORIZED, "Account not found".to_string()))?;

    if state.store.find_user_by_email(&req.email).await.is_some() {
        return Err((StatusCode::CONFLICT, "Email already in use".to_string()));
    }

    state.store.set_email(&wallet, &req.email).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    info!("[Auth] Added email {} to wallet {}", req.email, wallet);
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ── POST /auth/sync-profile ────────────────────────────────────────────────────

/// Minimal mirror of the on-chain PlayerProfile used only for borsh decoding.
/// Field order MUST match the Anchor account definition exactly.
#[derive(borsh::BorshDeserialize)]
struct ProfileOnChain {
    pub _authority:       [u8; 32],
    pub _country:         String,
    pub _wins:            u32,
    pub _losses:          u32,
    pub _draws:           u32,
    pub _games_played:    u32,
    pub _elo_rating:      f64,
    pub _rd:              f64,
    pub _volatility:      f64,
    pub _last_played:     i64,
    pub _win_streak:      u32,
    pub _best_streak:     u32,
    pub _tournament_wins: u32,
    pub _ranked_games:    u32,
    pub _total_wagered:   u64,
    pub _total_won:       u64,
    pub _created_at:      i64,
    pub _last_game_at:    i64,
    pub _is_verified:     bool,
    pub _annual_wins_gbp: u64,
    pub _annual_wins_brl: u64,
    pub _annual_wins_cad: u64,
    pub _annual_wins_eur: u64,
    pub username:         String,
    pub username_set:     bool,
}

/// POST /auth/sync-profile — reads the caller's on-chain PlayerProfile PDA,
/// extracts the canonical username, and writes it back to the SQLite DB.
/// Requires a valid Bearer JWT.  Safe to retry — idempotent.
async fn sync_profile(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // 1. Validate JWT → wallet pubkey
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(crate::signing::auth::extract_bearer)
        .ok_or((StatusCode::UNAUTHORIZED, "Missing Authorization header".to_string()))?;

    let claims = state.jwt.verify(token)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid or expired token".to_string()))?;
    let wallet = &claims.sub;

    // 2. Derive PlayerProfile PDA  (seeds: ["profile", wallet_bytes])
    let wallet_pk = Pubkey::from_str(wallet)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid wallet in token".to_string()))?;
    let program_id = Pubkey::from_str(&state.config.program_id)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Invalid PROGRAM_ID config".to_string()))?;
    let (profile_pda, _) = solana_sdk::pubkey::Pubkey::find_program_address(
        &[b"profile", wallet_pk.as_ref()],
        &program_id,
    );

    // 3. Fetch account data from Solana RPC (uses SOLANA_RPC_URL — correct in prod)
    let rpc = solana_client::nonblocking::rpc_client::RpcClient::new(
        state.config.solana_rpc_url.clone(),
    );
    let account = rpc.get_account(&profile_pda).await
        .map_err(|_| (StatusCode::NOT_FOUND, "On-chain profile not found. Create one first.".to_string()))?;

    // 4. Borsh-decode: skip 8-byte Anchor discriminator then deserialise
    if account.data.len() < 9 {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Account data too short".to_string()));
    }
    let profile = ProfileOnChain::try_from_slice(&account.data[8..])
        .map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, format!("Failed to decode profile: {e}")))?;

    if !profile.username_set || profile.username.is_empty() {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "No username set on-chain yet".to_string()));
    }

    // 5. Update SQLite — this is now the canonical username
    state.store.update_username(wallet, &profile.username).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    info!("[Auth] Synced on-chain username '{}' for {}", profile.username, wallet);
    Ok(Json(serde_json::json!({ "username": profile.username })))
}

// ── PATCH /auth/username ──────────────────────────────────────────────────────

#[derive(Deserialize)]
struct SetUsernameReq {
    username: String,
}

/// PATCH /auth/username — updates the display username in SQLite for the JWT's wallet.
/// Checks availability then writes. Does not touch the on-chain account.
async fn set_username(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<SetUsernameReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(crate::signing::auth::extract_bearer)
        .ok_or((StatusCode::UNAUTHORIZED, "Missing Authorization header".to_string()))?;

    let claims = state.jwt.verify(token)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid or expired token".to_string()))?;

    if req.username.len() < 3 || req.username.len() > 20 {
        return Err((StatusCode::BAD_REQUEST, "Username must be 3-20 characters".to_string()));
    }

    if state.store.username_taken(&req.username).await {
        return Err((StatusCode::CONFLICT, "Username already taken".to_string()));
    }

    state.store.update_username(&claims.sub, &req.username).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    info!("[Auth] Username updated to '{}' for {}", req.username, claims.sub);
    Ok(Json(serde_json::json!({ "username": req.username })))
}

// ── Check username ─────────────────────────────────────────────────────────────

async fn check_username(
    State(state): State<AppState>,
    axum::extract::Path(username): axum::extract::Path<String>,
) -> Json<serde_json::Value> {
    let taken = state.store.username_taken(&username).await;
    Json(serde_json::json!({ "taken": taken }))
}

// ── Check wallet ───────────────────────────────────────────────────────────────

async fn check_wallet(
    State(state): State<AppState>,
    axum::extract::Path(wallet): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let user = state.store.find_user_by_wallet(&wallet).await;
    let registered = user.is_some();
    if registered {
        Ok(Json(serde_json::json!({ "registered": true, "username": user.unwrap().1 })))
    } else {
        Err((StatusCode::NOT_FOUND, "Wallet not registered".to_string()))
    }
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
