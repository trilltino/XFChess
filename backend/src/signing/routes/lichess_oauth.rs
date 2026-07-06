//! Lichess OAuth 2.0 + PKCE integration for external ELO linking.
//!
//! Replaces the bio-nonce flow for production use. Bio-nonce remains as a
//! manual fallback in external_elo.rs.
//!
//! Endpoints:
//! - GET  /api/auth/lichess/init?wallet_pubkey=... — start OAuth flow
//! - POST /api/auth/lichess/exchange — exchange code for token + link on-chain

use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use solana_sdk::{pubkey::Pubkey, signature::Signer};
use std::{
    collections::HashMap,
    str::FromStr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tracing::{error, info, warn};

use crate::signing::{solana, AppState};

// ── Request / Response types ─────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct InitRequest {
    pub wallet_pubkey: String,
}

#[derive(Serialize)]
pub struct InitResponse {
    pub auth_url: String,
    pub state: String,
    pub code_challenge: String,
}

#[derive(Deserialize)]
pub struct ExchangeRequest {
    pub code: String,
    pub state: String,
    pub code_verifier: String,
    pub wallet_pubkey: String,
}

#[derive(Serialize)]
pub struct ExchangeResponse {
    pub tx_signature: String,
    pub lichess_username: String,
    pub blitz_rating: u32,
    pub rapid_rating: u32,
    pub bullet_rating: u32,
    pub seeded_elo: f64,
}

#[derive(Serialize)]
pub struct LichessErrorResponse {
    pub error: String,
}

// ── PKCE State Store ─────────────────────────────────────────────────────────

#[derive(Clone)]
struct PkceState {
    code_verifier: String,
    wallet_pubkey: String,
    created_at: Instant,
}

use once_cell::sync::Lazy;

static PKCE_STATES: Lazy<Arc<Mutex<HashMap<String, PkceState>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

/// Creates the Lichess OAuth routes router.
pub fn lichess_oauth_routes() -> Router<AppState> {
    Router::new()
        .route("/auth/lichess/init", get(init_oauth))
        .route("/auth/lichess/exchange", post(exchange_code))
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// GET /api/auth/lichess/init?wallet_pubkey=...
/// Generates PKCE params, stores state, returns the Lichess authorize URL.
async fn init_oauth(
    State(state): State<AppState>,
    Query(req): Query<InitRequest>,
) -> Result<Json<InitResponse>, (StatusCode, String)> {
    // Validate wallet pubkey
    let _ = Pubkey::from_str(&req.wallet_pubkey).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("Invalid wallet_pubkey: {}", e),
        )
    })?;

    let client_id = &state.config.lichess_client_id;
    if client_id.is_empty() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Lichess OAuth not configured (missing LICHESS_CLIENT_ID)".to_string(),
        ));
    }

    // Generate PKCE parameters
    let code_verifier = generate_code_verifier();
    let code_challenge = code_challenge(&code_verifier);
    let state_param = generate_state();

    // Store PKCE state with wallet binding and 10-minute TTL
    {
        let mut store = PKCE_STATES
            .lock()
            .expect("PKCE mutex should not be poisoned");
        store.insert(
            state_param.clone(),
            PkceState {
                code_verifier: code_verifier.clone(),
                wallet_pubkey: req.wallet_pubkey.clone(),
                created_at: Instant::now(),
            },
        );
        // Clean expired entries while we have the lock
        store.retain(|_, v| v.created_at.elapsed() < Duration::from_secs(600));
    }

    // Build Lichess authorize URL
    let redirect_uri = "http://178.104.55.19/auth/lichess/callback";
    let auth_url = format!(
        "https://lichess.org/oauth?response_type=code&client_id={}&redirect_uri={}&state={}&code_challenge={}&code_challenge_method=S256&scope=preference:read",
        urlencoding::encode(client_id),
        urlencoding::encode(redirect_uri),
        urlencoding::encode(&state_param),
        urlencoding::encode(&code_challenge)
    );

    info!(
        "[LichessOAuth] Init flow for {} -> state={}",
        req.wallet_pubkey, state_param
    );

    Ok(Json(InitResponse {
        auth_url,
        state: state_param,
        code_challenge,
    }))
}

/// POST /api/auth/lichess/exchange
/// Exchanges the authorization code for an access token, fetches profile,
/// and submits link_external_elo on-chain.
async fn exchange_code(
    State(state): State<AppState>,
    Json(req): Json<ExchangeRequest>,
) -> Result<Json<ExchangeResponse>, (StatusCode, String)> {
    // Retrieve and validate PKCE state
    let pkce_state = {
        let mut store = PKCE_STATES
            .lock()
            .expect("PKCE mutex should not be poisoned");
        let entry = store.remove(&req.state).ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid or expired state parameter".to_string(),
            )
        })?;
        if entry.created_at.elapsed() > Duration::from_secs(600) {
            return Err((StatusCode::BAD_REQUEST, "State expired".to_string()));
        }
        if entry.wallet_pubkey != req.wallet_pubkey {
            return Err((
                StatusCode::FORBIDDEN,
                "State/wallet mismatch — possible CSRF attempt".to_string(),
            ));
        }
        entry
    };

    // Verify code_verifier matches what we stored
    if req.code_verifier != pkce_state.code_verifier {
        return Err((StatusCode::FORBIDDEN, "Invalid code_verifier".to_string()));
    }

    let client_id = &state.config.lichess_client_id;
    if client_id.is_empty() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Lichess OAuth not configured".to_string(),
        ));
    }

    // ── Step 1: Exchange code for access token ───────────────────────────────
    let token_url = "https://lichess.org/api/token";
    let redirect_uri = "http://178.104.55.19/auth/lichess/callback";

    let client = reqwest::Client::new();
    let token_resp = client
        .post(token_url)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", &req.code),
            ("code_verifier", &pkce_state.code_verifier),
            ("client_id", client_id),
            ("redirect_uri", redirect_uri),
        ])
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                format!("Token request failed: {}", e),
            )
        })?;

    if !token_resp.status().is_success() {
        let body = token_resp.text().await.unwrap_or_default();
        return Err((
            StatusCode::BAD_GATEWAY,
            format!("Lichess token error: {}", body),
        ));
    }

    let token_data: serde_json::Value = token_resp.json().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            format!("Failed to parse token response: {}", e),
        )
    })?;

    let access_token = token_data
        .get("access_token")
        .and_then(|t| t.as_str())
        .ok_or_else(|| {
            (
                StatusCode::BAD_GATEWAY,
                "Missing access_token in response".to_string(),
            )
        })?;

    info!(
        "[LichessOAuth] Got access token for wallet {}",
        req.wallet_pubkey
    );

    // ── Step 2: Fetch authenticated user profile ─────────────────────────────
    let profile_resp = client
        .get("https://lichess.org/api/account")
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                format!("Profile request failed: {}", e),
            )
        })?;

    if !profile_resp.status().is_success() {
        let body = profile_resp.text().await.unwrap_or_default();
        return Err((
            StatusCode::BAD_GATEWAY,
            format!("Lichess profile error: {}", body),
        ));
    }

    let profile: serde_json::Value = profile_resp.json().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            format!("Failed to parse profile response: {}", e),
        )
    })?;

    let username = profile
        .get("username")
        .and_then(|u| u.as_str())
        .ok_or_else(|| {
            (
                StatusCode::BAD_GATEWAY,
                "Profile missing username".to_string(),
            )
        })?;

    let perfs = profile
        .get("perfs")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    let extract_rating = |key: &str| -> u32 {
        perfs
            .get(key)
            .and_then(|p| p.get("rating"))
            .and_then(|r| r.as_u64())
            .unwrap_or(0) as u32
    };

    let blitz_rating = extract_rating("blitz");
    let rapid_rating = extract_rating("rapid");
    let bullet_rating = extract_rating("bullet");

    info!(
        "[LichessOAuth] Fetched profile for {}: {} (Blitz: {}, Rapid: {}, Bullet: {})",
        req.wallet_pubkey, username, blitz_rating, rapid_rating, bullet_rating
    );

    // ── Step 3: Build and submit on-chain link_external_elo instruction ────
    let player_pk = Pubkey::from_str(&req.wallet_pubkey).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("Invalid wallet_pubkey: {}", e),
        )
    })?;
    let program_id = state.program_id;
    let link_authority = &state.link_authority;

    let ix = solana::link_external_elo_ix(
        &program_id,
        &link_authority.pubkey(),
        &player_pk,
        username,
        blitz_rating * 100, // centiscale
        rapid_rating * 100,
        bullet_rating * 100,
    );

    let rpc = solana::make_rpc(&state.solana_rpc_url);
    let tx_sig = match solana::sign_and_submit(&rpc, link_authority, &[ix]) {
        Ok(sig) => sig.to_string(),
        Err(e) => {
            error!(
                "[LichessOAuth] On-chain submission failed for {}: {}",
                req.wallet_pubkey, e
            );
            return Err((
                StatusCode::BAD_GATEWAY,
                format!("On-chain submission failed: {}", e),
            ));
        }
    };

    // ── Step 4: Persist to backend DB ──────────────────────────────────────
    let pool = state.store.pool();
    if let Err(e) = store_link_in_db(
        pool,
        &req.wallet_pubkey,
        username,
        blitz_rating,
        rapid_rating,
        bullet_rating,
        &tx_sig,
    )
    .await
    {
        warn!("[LichessOAuth] Failed to store link in DB: {}", e);
    }

    // Invalidate ELO cache
    state.elo_cache.invalidate(&req.wallet_pubkey);

    let seeded_elo = if blitz_rating > rapid_rating + 500 {
        blitz_rating as f64
    } else {
        rapid_rating as f64
    };

    info!(
        "[LichessOAuth] Linked {} -> Lichess '{}' (Blitz: {}, Rapid: {}, Bullet: {}) tx: {}",
        req.wallet_pubkey, username, blitz_rating, rapid_rating, bullet_rating, tx_sig
    );

    Ok(Json(ExchangeResponse {
        tx_signature: tx_sig,
        lichess_username: username.to_string(),
        blitz_rating,
        rapid_rating,
        bullet_rating,
        seeded_elo,
    }))
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Generates a PKCE code_verifier: 128 random bytes → base64url (no padding).
fn generate_code_verifier() -> String {
    use base64::{engine::general_purpose, Engine as _};
    use rand::RngCore;
    let mut bytes = vec![0u8; 128];
    rand::rng().fill_bytes(&mut bytes);
    general_purpose::URL_SAFE_NO_PAD.encode(&bytes)
}

/// Computes the PKCE code_challenge: SHA256(code_verifier) → base64url (no padding).
fn code_challenge(verifier: &str) -> String {
    use base64::{engine::general_purpose, Engine as _};
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(verifier.as_bytes());
    general_purpose::URL_SAFE_NO_PAD.encode(hash)
}

/// Generates a random state parameter for CSRF protection.
fn generate_state() -> String {
    use rand::RngCore;
    let mut bytes = vec![0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

// Reuse the DB helper from external_elo.rs — identical schema
async fn store_link_in_db(
    pool: sqlx::SqlitePool,
    pubkey: &str,
    username: &str,
    blitz: u32,
    rapid: u32,
    bullet: u32,
    tx_sig: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO external_elo_links (
            pubkey, platform, username, verified, blitz_rating, rapid_rating, bullet_rating,
            linked_at, last_sync_at, on_chain_tx
        ) VALUES (?, 'lichess', ?, 1, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(pubkey, platform) DO UPDATE SET
            username = excluded.username,
            verified = excluded.verified,
            blitz_rating = excluded.blitz_rating,
            rapid_rating = excluded.rapid_rating,
            bullet_rating = excluded.bullet_rating,
            last_sync_at = excluded.last_sync_at,
            on_chain_tx = excluded.on_chain_tx
        "#,
    )
    .bind(pubkey)
    .bind(username)
    .bind(blitz as i64)
    .bind(rapid as i64)
    .bind(bullet as i64)
    .bind(chrono::Utc::now().timestamp())
    .bind(chrono::Utc::now().timestamp())
    .bind(tx_sig)
    .execute(&pool)
    .await?;

    Ok(())
}
