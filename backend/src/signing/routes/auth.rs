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

use crate::signing::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use borsh::BorshDeserialize;
use serde::{Deserialize, Serialize};
use solana_sdk::{pubkey::Pubkey, signature::Signature};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

/// Maximum age (seconds) a signed `timestamp` may have before the signature is
/// rejected. Without this bound a captured signature is replayable forever,
/// which is an account-takeover primitive for `login`/`register`/`delete`.
const AUTH_SIG_MAX_AGE_SECS: u64 = 300; // 5 minutes
/// Allowance for the client's clock running ahead of the server.
const AUTH_SIG_FUTURE_SKEW_SECS: u64 = 60;

/// Verifies a wallet signature over `"xfchess:<action>:<timestamp>"`.
///
/// The `timestamp` must be recent (within [`AUTH_SIG_MAX_AGE_SECS`]) to defeat
/// replay of an old, legitimately-signed message. Returns `Err` with an
/// appropriate HTTP status on failure.
fn verify_wallet_sig(
    wallet: &str,
    signature: &str,
    action: &str,
    timestamp: u64,
) -> Result<Pubkey, (StatusCode, String)> {
    // Reject stale or far-future timestamps before doing crypto work.
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if timestamp > now.saturating_add(AUTH_SIG_FUTURE_SKEW_SECS)
        || now.saturating_sub(timestamp) > AUTH_SIG_MAX_AGE_SECS
    {
        return Err((
            StatusCode::UNAUTHORIZED,
            "Signature timestamp expired or invalid — re-sign with a current timestamp".to_string(),
        ));
    }

    let pk = Pubkey::from_str(wallet).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid wallet address".to_string(),
        )
    })?;
    let sig = Signature::from_str(signature).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid signature format".to_string(),
        )
    })?;
    let msg = format!("xfchess:{}:{}", action, timestamp);
    if !sig.verify(pk.as_ref(), msg.as_bytes()) {
        return Err((
            StatusCode::UNAUTHORIZED,
            "Signature verification failed".to_string(),
        ));
    }
    Ok(pk)
}

/// Authenticates a Bearer JWT request: verifies the token signature/expiry and
/// checks it against the per-subject revocation cut-off. Returns the wallet
/// (the `sub` claim) on success.
async fn authed_wallet(
    state: &AppState,
    headers: &axum::http::HeaderMap,
) -> Result<String, (StatusCode, String)> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(crate::signing::auth::extract_bearer)
        .ok_or((
            StatusCode::UNAUTHORIZED,
            "Missing Authorization header".to_string(),
        ))?;

    let claims = state.jwt.verify(token).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            "Invalid or expired token".to_string(),
        )
    })?;

    if state.store.token_is_revoked(&claims.sub, claims.iat).await {
        return Err((
            StatusCode::UNAUTHORIZED,
            "Token revoked — please log in again".to_string(),
        ));
    }

    Ok(claims.sub)
}

/// Creates the authentication router.
pub fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/logout", post(logout))
        .route("/register", post(register))
        .route("/register-email", post(register_email))
        .route("/login", post(login))
        .route("/login-email", post(login_email))
        .route("/link-wallet", post(link_wallet))
        .route("/me", get(me))
        .route("/add-email", post(add_email))
        .route("/sync-profile", post(sync_profile))
        .route("/init-profile-tx", post(init_profile_tx))
        .route("/broadcast-tx", post(broadcast_tx))
        .route("/username", axum::routing::patch(set_username))
        .route("/check-username/{username}", get(check_username))
        .route("/check-wallet/{wallet}", get(check_wallet))
        .route("/delete", post(delete_account))
        .route("/siws-challenge", post(siws_challenge))
        .route("/siws-verify", post(siws_verify))
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
        return Err((
            StatusCode::CONFLICT,
            "Wallet already registered".to_string(),
        ));
    }
    if state.store.username_taken(&req.username).await {
        return Err((StatusCode::CONFLICT, "Username already taken".to_string()));
    }

    state
        .store
        .create_wallet_user(&req.wallet, &req.username, req.email.as_deref())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let token = state
        .jwt
        .issue(&req.wallet)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    info!("[Auth] Registered wallet: {}", req.wallet);
    Ok(Json(AuthResp {
        token,
        username: req.username,
        wallet: req.wallet,
    }))
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

    let user = state.store.find_user_by_wallet(&req.wallet).await.ok_or((
        StatusCode::UNAUTHORIZED,
        "Wallet not registered. Please create an account first.".to_string(),
    ))?;

    let token = state
        .jwt
        .issue(&req.wallet)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    info!("[Auth] Login: {}", req.wallet);
    Ok(Json(AuthResp {
        token,
        username: user.1,
        wallet: req.wallet,
    }))
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

    let token = state
        .jwt
        .issue(&format!("email:{}", req.email))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    info!("[Auth] Registered email user: {}", req.email);
    Ok(Json(AuthResp {
        token,
        username: req.username,
        wallet: "".to_string(),
    }))
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
    let user = state.store.find_user_by_email(&req.email).await.ok_or((
        StatusCode::UNAUTHORIZED,
        "Invalid email or password".to_string(),
    ))?;

    let stored_hash = user.4.ok_or((
        StatusCode::UNAUTHORIZED,
        "This account does not have a password. Please login with wallet.".to_string(),
    ))?;

    use argon2::{password_hash::PasswordHash, password_hash::PasswordVerifier, Argon2};
    let parsed_hash = PasswordHash::new(&stored_hash)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Argon2::default()
        .verify_password(req.password.as_bytes(), &parsed_hash)
        .map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                "Invalid email or password".to_string(),
            )
        })?;

    let identity = if !user.0.is_empty() {
        user.0.clone()
    } else {
        format!("email:{}", req.email)
    };

    let token = state
        .jwt
        .issue(&identity)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    info!("[Auth] Login email: {}", req.email);
    Ok(Json(AuthResp {
        token,
        username: user.1,
        wallet: user.0,
    }))
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
    let user = state.store.find_user_by_email(&req.email).await.ok_or((
        StatusCode::UNAUTHORIZED,
        "Invalid email or password".to_string(),
    ))?;

    let stored_hash = user.4.ok_or((
        StatusCode::BAD_REQUEST,
        "Account has no password".to_string(),
    ))?;
    use argon2::{password_hash::PasswordHash, password_hash::PasswordVerifier, Argon2};
    let parsed_hash = PasswordHash::new(&stored_hash)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Argon2::default()
        .verify_password(req.password.as_bytes(), &parsed_hash)
        .map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                "Invalid email or password".to_string(),
            )
        })?;

    // 3. Link Wallet
    state
        .store
        .link_wallet(&req.email, &req.wallet)
        .await
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
    /// True when the wallet has an initialised on-chain PlayerProfile PDA.
    has_onchain_profile: bool,
    /// ELO from the VPS backend (0 = unranked).
    elo: u32,
    /// ISO 3166-1 alpha-2 country from VPS record (empty if not set).
    country: String,
}

/// GET /auth/me — validates Bearer JWT and returns caller profile.
async fn me(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<MeResp>, (StatusCode, String)> {
    let wallet = authed_wallet(&state, &headers).await?;

    let user = state
        .store
        .find_user_by_wallet(&wallet)
        .await
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

    // Check whether an on-chain PlayerProfile PDA exists for this wallet.
    let has_onchain_profile = if wallet_linked {
        let rpc = std::sync::Arc::clone(&state.solana_rpc);
        let wallet_pk = wallet.clone();
        let program_id = state.program_id;
        tokio::task::spawn_blocking(move || {
            use std::str::FromStr;
            let pubkey = solana_sdk::pubkey::Pubkey::from_str(&wallet_pk).ok()?;
            let (profile_pda, _) = solana_sdk::pubkey::Pubkey::find_program_address(
                &[b"profile", pubkey.as_ref()],
                &program_id,
            );
            rpc.get_account(&profile_pda).ok().map(|_| true)
        })
        .await
        .ok()
        .flatten()
        .unwrap_or(false)
    } else {
        false
    };

    // Pull ELO from on-chain cache (non-fatal if missing or no profile yet).
    let cached_elo = state.elo_cache.get_elo(&wallet).await.ok();
    let elo = cached_elo
        .as_ref()
        .map(|e| e.elo_rating as u32)
        .unwrap_or(0);
    let country = kyc_country.unwrap_or_default();

    Ok(Json(MeResp {
        wallet: user.0,
        username: user.1,
        email: user.2,
        kyc_status: user.3,
        wallet_linked,
        can_wager,
        has_onchain_profile,
        elo,
        country,
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
    let wallet = authed_wallet(&state, &headers).await?;

    state
        .store
        .find_user_by_wallet(&wallet)
        .await
        .ok_or((StatusCode::UNAUTHORIZED, "Account not found".to_string()))?;

    if state.store.find_user_by_email(&req.email).await.is_some() {
        return Err((StatusCode::CONFLICT, "Email already in use".to_string()));
    }

    state
        .store
        .set_email(&wallet, &req.email)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    info!("[Auth] Added email {} to wallet {}", req.email, wallet);
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ── POST /auth/sync-profile ────────────────────────────────────────────────────

/// Minimal mirror of the on-chain PlayerProfile used only for borsh decoding.
/// Field order MUST match the Anchor account definition exactly.
#[derive(borsh::BorshDeserialize)]
struct ProfileOnChain {
    pub _authority: [u8; 32],
    pub _country: String,
    pub _wins: u32,
    pub _losses: u32,
    pub _draws: u32,
    pub _games_played: u32,
    pub _elo_rating: f64,
    pub _rd: f64,
    pub _volatility: f64,
    pub _last_played: i64,
    pub _win_streak: u32,
    pub _best_streak: u32,
    pub _tournament_wins: u32,
    pub _ranked_games: u32,
    pub _total_wagered: u64,
    pub _total_won: u64,
    pub _created_at: i64,
    pub _last_game_at: i64,
    pub _is_verified: bool,
    pub _annual_wins_gbp: u64,
    pub _annual_wins_brl: u64,
    pub _annual_wins_cad: u64,
    pub _annual_wins_eur: u64,
    pub username: String,
    pub username_set: bool,
}

/// POST /auth/sync-profile — reads the caller's on-chain PlayerProfile PDA,
/// extracts the canonical username, and writes it back to the SQLite DB.
/// Requires a valid Bearer JWT.  Safe to retry — idempotent.
async fn sync_profile(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // 1. Validate JWT → wallet pubkey
    let wallet = authed_wallet(&state, &headers).await?;
    let wallet = &wallet;

    // 2. Derive PlayerProfile PDA  (seeds: ["profile", wallet_bytes])
    let wallet_pk = Pubkey::from_str(wallet).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid wallet in token".to_string(),
        )
    })?;
    let program_id = Pubkey::from_str(&state.config.program_id).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Invalid PROGRAM_ID config".to_string(),
        )
    })?;
    let (profile_pda, _) = solana_sdk::pubkey::Pubkey::find_program_address(
        &[b"profile", wallet_pk.as_ref()],
        &program_id,
    );

    // 3. Fetch account data from Solana RPC (uses SOLANA_RPC_URL — correct in prod)
    let rpc =
        solana_client::nonblocking::rpc_client::RpcClient::new(state.config.solana_rpc_url.clone());
    let account = rpc.get_account(&profile_pda).await.map_err(|_| {
        (
            StatusCode::NOT_FOUND,
            "On-chain profile not found. Create one first.".to_string(),
        )
    })?;

    // 4. Borsh-decode: skip 8-byte Anchor discriminator then deserialise
    if account.data.len() < 9 {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "Account data too short".to_string(),
        ));
    }
    let profile = ProfileOnChain::try_from_slice(&account.data[8..]).map_err(|e| {
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("Failed to decode profile: {e}"),
        )
    })?;

    if !profile.username_set || profile.username.is_empty() {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "No username set on-chain yet".to_string(),
        ));
    }

    // 5. Update SQLite — this is now the canonical username
    state
        .store
        .update_username(wallet, &profile.username)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    info!(
        "[Auth] Synced on-chain username '{}' for {}",
        profile.username, wallet
    );
    Ok(Json(serde_json::json!({ "username": profile.username })))
}

// ── POST /auth/init-profile-tx ────────────────────────────────────────────────

/// Builds an unsigned `initProfile` transaction and returns it as base64.
/// The client signs with their wallet then broadcasts via Solana RPC.
///
/// Anchor instruction discriminator: sha256("global:init_profile")[0..8]
/// = [0xd2, 0xa2, 0xd4, 0x5f, 0x5f, 0xba, 0x59, 0x77]
#[derive(Deserialize)]
struct InitProfileTxReq {
    username: String,
    country: String,
    /// Unix timestamp (seconds). Must be ≥ 18 years before now.
    date_of_birth: i64,
}

#[derive(Serialize)]
struct InitProfileTxResp {
    tx_b64: String,
    profile_pda: String,
}

async fn init_profile_tx(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<InitProfileTxReq>,
) -> Result<Json<InitProfileTxResp>, (StatusCode, String)> {
    use base64::engine::general_purpose;
    use base64::Engine as _;
    use solana_sdk::{
        instruction::{AccountMeta, Instruction},
        system_program,
        transaction::Transaction,
    };

    // Validate JWT → wallet pubkey
    let wallet = authed_wallet(&state, &headers).await?;
    let wallet_pk = Pubkey::from_str(&wallet).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid wallet in token".to_string(),
        )
    })?;

    // Validate inputs
    if req.username.len() < 3 || req.username.len() > 20 {
        return Err((
            StatusCode::BAD_REQUEST,
            "Username must be 3–20 chars".to_string(),
        ));
    }
    let min_dob = chrono::Utc::now().timestamp() - 567_648_000; // 18 years
    if req.date_of_birth <= 0 || req.date_of_birth > min_dob {
        return Err((StatusCode::BAD_REQUEST, "Must be 18+ years old".to_string()));
    }

    let program_id = state.program_id;

    // Derive PDAs
    let (profile_pda, _) =
        Pubkey::find_program_address(&[b"profile", wallet_pk.as_ref()], &program_id);
    let (username_record_pda, _) =
        Pubkey::find_program_address(&[b"username", req.username.as_bytes()], &program_id);

    // Build instruction data: discriminator + borsh(username, country, date_of_birth)
    // Borsh string: 4-byte LE length prefix + UTF-8 bytes
    // Borsh i64: 8-byte LE
    let discriminator: [u8; 8] = [0xd2, 0xa2, 0xd4, 0x5f, 0x5f, 0xba, 0x59, 0x77];
    let mut data = Vec::with_capacity(64);
    data.extend_from_slice(&discriminator);
    // username
    let un_bytes = req.username.as_bytes();
    data.extend_from_slice(&(un_bytes.len() as u32).to_le_bytes());
    data.extend_from_slice(un_bytes);
    // country
    let co_bytes = req.country.as_bytes();
    data.extend_from_slice(&(co_bytes.len() as u32).to_le_bytes());
    data.extend_from_slice(co_bytes);
    // date_of_birth (i64 LE)
    data.extend_from_slice(&req.date_of_birth.to_le_bytes());

    let accounts = vec![
        AccountMeta::new(profile_pda, false),
        AccountMeta::new(username_record_pda, false),
        AccountMeta::new(wallet_pk, true),
        AccountMeta::new_readonly(system_program::id(), false),
    ];

    let ix = Instruction {
        program_id,
        accounts,
        data,
    };

    // Fetch a recent blockhash so the transaction is immediately broadcastable.
    let rpc = std::sync::Arc::clone(&state.solana_rpc);
    let recent_blockhash = tokio::task::spawn_blocking(move || rpc.get_latest_blockhash())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("RPC blockhash: {e}")))?;

    let tx = Transaction::new_unsigned(solana_sdk::message::Message::new_with_blockhash(
        &[ix],
        Some(&wallet_pk),
        &recent_blockhash,
    ));
    let tx_bytes = bincode::serialize(&tx)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("serialize: {e}")))?;
    let tx_b64 = general_purpose::STANDARD.encode(&tx_bytes);

    info!(
        "[Auth] Built init_profile_tx for {} username={}",
        wallet_pk, req.username
    );
    Ok(Json(InitProfileTxResp {
        tx_b64,
        profile_pda: profile_pda.to_string(),
    }))
}

// ── POST /auth/broadcast-tx ───────────────────────────────────────────────────

/// Broadcast a signed and serialised transaction (bincode base64) to Solana.
/// Returns the transaction signature on success.
#[derive(Deserialize)]
struct BroadcastTxReq {
    /// Base64-encoded bincode-serialised signed Transaction.
    tx_b64: String,
}

#[derive(Serialize)]
struct BroadcastTxResp {
    signature: String,
}

async fn broadcast_tx(
    State(state): State<AppState>,
    Json(req): Json<BroadcastTxReq>,
) -> Result<Json<BroadcastTxResp>, (StatusCode, String)> {
    use base64::engine::general_purpose;
    use base64::Engine as _;
    use solana_client::rpc_config::RpcSendTransactionConfig;
    use solana_sdk::commitment_config::CommitmentConfig;
    use solana_sdk::transaction::Transaction;

    let tx_bytes = general_purpose::STANDARD
        .decode(&req.tx_b64)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("base64 decode: {e}")))?;

    let tx: Transaction = bincode::deserialize(&tx_bytes)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("deserialize tx: {e}")))?;

    let rpc = std::sync::Arc::clone(&state.solana_rpc);
    let sig = tokio::task::spawn_blocking(move || {
        rpc.send_and_confirm_transaction_with_spinner_and_config(
            &tx,
            CommitmentConfig::confirmed(),
            RpcSendTransactionConfig {
                skip_preflight: false,
                ..Default::default()
            },
        )
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::BAD_GATEWAY, format!("RPC broadcast: {e}")))?;

    info!("[Auth] Broadcast tx: {sig}");
    Ok(Json(BroadcastTxResp {
        signature: sig.to_string(),
    }))
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
    let wallet = authed_wallet(&state, &headers).await?;

    if req.username.len() < 3 || req.username.len() > 20 {
        return Err((
            StatusCode::BAD_REQUEST,
            "Username must be 3-20 characters".to_string(),
        ));
    }

    if state.store.username_taken(&req.username).await {
        return Err((StatusCode::CONFLICT, "Username already taken".to_string()));
    }

    state
        .store
        .update_username(&wallet, &req.username)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    info!(
        "[Auth] Username updated to '{}' for {}",
        req.username, wallet
    );
    Ok(Json(serde_json::json!({ "username": req.username })))
}

// ── POST /auth/logout ──────────────────────────────────────────────────────────

/// POST /auth/logout — revokes every JWT previously issued to the caller.
/// Requires a valid Bearer JWT. After this, the presented token (and any other
/// outstanding token for the same wallet) is rejected until the user logs in
/// again, giving JWTs a server-side kill switch.
async fn logout(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let wallet = authed_wallet(&state, &headers).await?;
    let now = chrono::Utc::now().timestamp();
    state
        .store
        .revoke_tokens_before(&wallet, now)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    info!("[Auth] Logout — revoked tokens for {}", wallet);
    Ok(Json(serde_json::json!({ "ok": true })))
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
        Ok(Json(
            serde_json::json!({ "registered": true, "username": user.unwrap().1 }),
        ))
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

    state
        .store
        .find_user_by_wallet(&req.wallet)
        .await
        .ok_or((StatusCode::NOT_FOUND, "Wallet not registered".to_string()))?;

    // 1. Erase auth record
    state
        .store
        .erase_user(&req.wallet)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 2. Erase KYC PII from vault and write audit trail
    let vault = crate::signing::storage::vault::VaultStore::new((*state.vault_pool).clone());
    let _ = vault.erase_kyc(&req.wallet).await;
    let _ = vault
        .log_deletion_request(&req.wallet, None, req.reason.as_deref())
        .await;
    let _ = vault.complete_deletion_request(&req.wallet).await;
    vault.write_audit(&req.wallet, "account_deleted").await;

    info!("[Auth] GDPR erasure: {}", req.wallet);
    Ok(Json(
        serde_json::json!({ "ok": true, "message": "Account and KYC data erased." }),
    ))
}

// ── SIWS (Sign-In With Solana) ─────────────────────────────────────────────────
//
// Headless wallet auth for the game client — no browser extension required.
// Flow:
//   1. POST /auth/siws-challenge  →  { nonce }
//   2. Client signs `"xfchess:siws:<nonce>"` with their wallet keypair
//   3. POST /auth/siws-verify { wallet, signature, nonce }  →  AuthResp (JWT)
//
// Nonces are one-time-use and expire after 5 minutes.

#[derive(Deserialize)]
struct SiwsChallengeReq {
    wallet: String,
}

#[derive(Deserialize)]
struct SiwsVerifyReq {
    wallet: String,
    signature: String,
    nonce: String,
}

/// POST /auth/siws-challenge — issues a one-time nonce for the wallet to sign.
async fn siws_challenge(
    State(state): State<AppState>,
    Json(req): Json<SiwsChallengeReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    Pubkey::from_str(&req.wallet).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid wallet address".to_string(),
        )
    })?;

    let nonce = uuid::Uuid::new_v4().to_string();
    let expires_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        + 300; // 5 minutes

    state
        .siws_nonces
        .lock()
        .await
        .insert(nonce.clone(), (req.wallet.clone(), expires_at));

    info!("[SIWS] challenge issued for {}", req.wallet);
    Ok(Json(serde_json::json!({ "nonce": nonce })))
}

/// POST /auth/siws-verify — verifies the signed nonce and returns a JWT.
async fn siws_verify(
    State(state): State<AppState>,
    Json(req): Json<SiwsVerifyReq>,
) -> Result<Json<AuthResp>, (StatusCode, String)> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Validate and consume the nonce
    let (nonce_wallet, expires_at) = {
        let mut map = state.siws_nonces.lock().await;
        map.remove(&req.nonce).ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                "Unknown or already-used nonce".to_string(),
            )
        })?
    };

    if now > expires_at {
        return Err((StatusCode::UNAUTHORIZED, "Nonce expired".to_string()));
    }
    if nonce_wallet != req.wallet {
        return Err((
            StatusCode::UNAUTHORIZED,
            "Wallet mismatch for nonce".to_string(),
        ));
    }

    // Verify signature over `xfchess:siws:<nonce>`
    let pk = Pubkey::from_str(&req.wallet).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid wallet address".to_string(),
        )
    })?;
    let sig = Signature::from_str(&req.signature).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid signature format".to_string(),
        )
    })?;
    let msg = format!("xfchess:siws:{}", req.nonce);
    if !sig.verify(pk.as_ref(), msg.as_bytes()) {
        return Err((
            StatusCode::UNAUTHORIZED,
            "Signature verification failed".to_string(),
        ));
    }

    // Ensure account exists (auto-create if first time)
    let username = if let Some(user) = state.store.find_user_by_wallet(&req.wallet).await {
        user.1
    } else {
        let default_username = req.wallet[..8.min(req.wallet.len())].to_string();
        let _ = state
            .store
            .create_wallet_user(&req.wallet, &default_username, None)
            .await;
        default_username
    };

    let token = state
        .jwt
        .issue(&req.wallet)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    info!("[SIWS] verified + JWT issued for {}", req.wallet);
    Ok(Json(AuthResp {
        token,
        username,
        wallet: req.wallet,
    }))
}
