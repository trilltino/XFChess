//! Wallet sign-in for the admin panel.
//!
//! The panel asks for a challenge, signs it with an admin wallet and exchanges the signature for
//! a short-lived session token. The token travels in the same `X-API-Key` header as an API key
//! (it starts with [`SESSION_PREFIX`]), so every admin route, the audit log and the panel's
//! downloads accept it without any other change.
//!
//! - `ADMIN_WALLETS` lists the wallets that may sign in: `name:PUBKEY,name2:PUBKEY2` (a bare
//!   pubkey is allowed; its name is the first eight characters). Unset or empty disables wallet
//!   sign-in and leaves API keys as the only way in.
//! - A challenge is single use, bound to the wallet it was issued for, and valid for two minutes.
//! - The signed text is built by the server from the stored challenge; the client never chooses it.
//! - Session tokens are HS256 JWTs signed with a key derived from `JWT_SECRET` that wallet (player)
//!   tokens do not share, so a player token is never an admin session.

use axum::{http::StatusCode, routing::post, Json, Router};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use solana_sdk::{pubkey::Pubkey, signature::Signature};
use std::collections::HashMap;
use std::env;
use std::str::FromStr;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

pub const SESSION_PREFIX: &str = "xfa.";
const CHALLENGE_TTL_SECS: u64 = 120;
const SESSION_TTL_SECS: u64 = 8 * 60 * 60;
const MAX_PENDING_CHALLENGES: usize = 256;
const SESSION_KIND: &str = "admin-session";

struct Pending {
    wallet: String,
    expires_at: u64,
}

fn pending() -> &'static Mutex<HashMap<String, Pending>> {
    static PENDING: OnceLock<Mutex<HashMap<String, Pending>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The configured admin wallets as `(name, pubkey)`.
fn admin_wallets() -> Vec<(String, Pubkey)> {
    let Ok(list) = env::var("ADMIN_WALLETS") else {
        return Vec::new();
    };
    list.split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .filter_map(|entry| {
            let (name, key) = match entry.split_once(':') {
                Some((name, key)) => (name.trim().to_string(), key.trim()),
                None => (entry.chars().take(8).collect::<String>(), entry),
            };
            let pubkey = Pubkey::from_str(key).ok()?;
            let name = if name.is_empty() {
                pubkey.to_string().chars().take(8).collect()
            } else {
                name
            };
            Some((name, pubkey))
        })
        .collect()
}

fn admin_name(wallet: &str) -> Option<String> {
    admin_wallets()
        .into_iter()
        .find(|(_, key)| key.to_string() == wallet)
        .map(|(name, _)| name)
}

fn session_secret() -> Option<String> {
    let secret = env::var("JWT_SECRET").ok().filter(|s| !s.is_empty())?;
    Some(format!("xfchess-admin-session|{secret}"))
}

#[derive(Serialize, Deserialize)]
struct SessionClaims {
    sub: String,
    name: String,
    kind: String,
    iat: u64,
    exp: u64,
}

fn issue_session(wallet: &str, name: &str) -> Option<(String, u64)> {
    let secret = session_secret()?;
    let issued = now();
    let expires = issued + SESSION_TTL_SECS;
    let claims = SessionClaims {
        sub: wallet.to_string(),
        name: name.to_string(),
        kind: SESSION_KIND.to_string(),
        iat: issued,
        exp: expires,
    };
    let jwt = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .ok()?;
    Some((format!("{SESSION_PREFIX}{jwt}"), expires))
}

/// The actor name for a valid session token, checked against the current allowlist so removing a
/// wallet from `ADMIN_WALLETS` ends its sessions at the next request.
pub fn actor_for_session(token: &str) -> Result<String, StatusCode> {
    let jwt = token
        .strip_prefix(SESSION_PREFIX)
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let secret = session_secret().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let mut validation = Validation::new(Algorithm::HS256);
    validation.leeway = 0;
    validation.set_required_spec_claims(&["exp", "sub"]);
    let data = decode::<SessionClaims>(
        jwt,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|_| StatusCode::UNAUTHORIZED)?;
    if data.claims.kind != SESSION_KIND {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let name = admin_name(&data.claims.sub).ok_or(StatusCode::UNAUTHORIZED)?;
    Ok(format!("wallet:{name}"))
}

fn challenge_message(wallet: &str, nonce: &str, expires_at: u64) -> String {
    format!("XFChess admin sign-in\nwallet: {wallet}\nnonce: {nonce}\nexpires: {expires_at}")
}

fn new_nonce() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

#[derive(Deserialize)]
struct ChallengeRequest {
    wallet: String,
}

#[derive(Serialize)]
struct ChallengeResponse {
    nonce: String,
    message: String,
    expires_at: u64,
}

async fn challenge(
    Json(req): Json<ChallengeRequest>,
) -> Result<Json<ChallengeResponse>, (StatusCode, &'static str)> {
    if admin_wallets().is_empty() || session_secret().is_none() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "wallet sign-in is not configured",
        ));
    }
    if Pubkey::from_str(&req.wallet).is_err() {
        return Err((StatusCode::BAD_REQUEST, "invalid wallet"));
    }
    if admin_name(&req.wallet).is_none() {
        return Err((StatusCode::FORBIDDEN, "this wallet is not an admin wallet"));
    }
    let issued = now();
    let expires_at = issued + CHALLENGE_TTL_SECS;
    let nonce = new_nonce();
    {
        let mut store = pending()
            .lock()
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "challenge store unavailable"))?;
        store.retain(|_, p| p.expires_at > issued);
        if store.len() >= MAX_PENDING_CHALLENGES {
            return Err((StatusCode::TOO_MANY_REQUESTS, "too many pending sign-ins"));
        }
        store.insert(
            nonce.clone(),
            Pending {
                wallet: req.wallet.clone(),
                expires_at,
            },
        );
    }
    Ok(Json(ChallengeResponse {
        message: challenge_message(&req.wallet, &nonce, expires_at),
        nonce,
        expires_at,
    }))
}

#[derive(Deserialize)]
struct LoginRequest {
    wallet: String,
    nonce: String,
    /// Base58 ed25519 signature of the challenge message.
    signature: String,
}

#[derive(Serialize)]
struct LoginResponse {
    token: String,
    actor: String,
    expires_at: u64,
}

async fn login(
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, &'static str)> {
    // Take the challenge out first: it is spent whether or not the signature turns out valid.
    let taken = pending()
        .lock()
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "challenge store unavailable"))?
        .remove(&req.nonce);
    let Some(challenge) = taken else {
        return Err((StatusCode::UNAUTHORIZED, "unknown or already used challenge"));
    };
    if challenge.expires_at <= now() {
        return Err((StatusCode::UNAUTHORIZED, "the challenge expired"));
    }
    if challenge.wallet != req.wallet {
        return Err((StatusCode::UNAUTHORIZED, "challenge issued to another wallet"));
    }
    let pubkey = Pubkey::from_str(&req.wallet).map_err(|_| (StatusCode::BAD_REQUEST, "invalid wallet"))?;
    let signature = Signature::from_str(&req.signature)
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid signature"))?;
    let message = challenge_message(&req.wallet, &req.nonce, challenge.expires_at);
    if !signature.verify(pubkey.as_ref(), message.as_bytes()) {
        return Err((StatusCode::UNAUTHORIZED, "signature does not match"));
    }
    let name = admin_name(&req.wallet)
        .ok_or((StatusCode::FORBIDDEN, "this wallet is not an admin wallet"))?;
    let (token, expires_at) = issue_session(&req.wallet, &name).ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "wallet sign-in is not configured",
    ))?;
    tracing::info!("[auth] admin wallet sign-in: {name}");
    Ok(Json(LoginResponse {
        token,
        actor: format!("wallet:{name}"),
        expires_at,
    }))
}

/// Mounted outside the admin guard: these two routes are how a caller obtains a credential.
pub fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/challenge", post(challenge))
        .route("/login", post(login))
}
