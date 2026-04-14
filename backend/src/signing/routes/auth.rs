use axum::{
    extract::State,
    http::StatusCode,
    Json, Router,
    routing::post,
};
use serde::{Deserialize, Serialize};
use tracing::{info, error};
use crate::signing::AppState;
use argon2::{
    password_hash::{
        rand_core::OsRng,
        PasswordHash, PasswordHasher, PasswordVerifier, SaltString
    },
    Argon2
};
use solana_sdk::{pubkey::Pubkey, signature::Signature};
use std::str::FromStr;

pub fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/login", post(login))
        .route("/register", post(register))
        .route("/link-wallet", post(link_wallet))
        .route("/login-wallet", post(login_wallet))
}

#[derive(Deserialize)]
pub struct AuthReq {
    pub email: String,
    pub password: String,
    pub username: Option<String>,
}

#[derive(Serialize)]
pub struct AuthResp {
    pub token: String,
    pub username: String,
    pub wallet: Option<String>,
}

async fn register(
    State(state): State<AppState>,
    Json(req): Json<AuthReq>,
) -> Result<Json<AuthResp>, (StatusCode, String)> {
    let username = req.username.ok_or((StatusCode::BAD_REQUEST, "Username required for registration".to_string()))?;
    
    // Check for existing users
    if state.store.find_user(&req.email).await.is_some() {
        return Err((StatusCode::CONFLICT, "User already exists".to_string()));
    }

    // ── ARGON2 HASHING ────────────────────────────────────────────────────────
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2.hash_password(req.password.as_bytes(), &salt)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Hashing failed: {}", e)))?
        .to_string();

    state.store.create_user(&req.email, &password_hash, &username).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Issue JWT for the email
    let token = state.jwt.issue(&req.email).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    info!("[Auth] Registered user: {}", req.email);

    Ok(Json(AuthResp {
        token,
        username,
        wallet: None,
    }))
}

async fn login(
    State(state): State<AppState>,
    Json(req): Json<AuthReq>,
) -> Result<Json<AuthResp>, (StatusCode, String)> {
    let user = state.store.find_user(&req.email).await
        .ok_or((StatusCode::UNAUTHORIZED, "Invalid email or password".to_string()))?;

    // ── ARGON2 VERIFICATION ───────────────────────────────────────────────────
    let stored_hash = user.1; // password_hash is the second field in the tuple
    let parsed_hash = PasswordHash::new(&stored_hash)
        .map_err(|e| {
            error!("Hash parsing error for {}: {}", req.email, e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Invalid password record structure".to_string())
        })?;

    if Argon2::default().verify_password(req.password.as_bytes(), &parsed_hash).is_err() {
        return Err((StatusCode::UNAUTHORIZED, "Invalid email or password".to_string()));
    }

    let token = state.jwt.issue(&req.email).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    info!("[Auth] Logged in user: {}", req.email);

    Ok(Json(AuthResp {
        token,
        username: user.2,
        wallet: user.3,
    }))
}

#[derive(Deserialize)]
pub struct LinkWalletReq {
    pub email: String,
    pub wallet: String,
}

async fn link_wallet(
    State(state): State<AppState>,
    Json(req): Json<LinkWalletReq>,
) -> Result<Json<()>, (StatusCode, String)> {
    state.store.link_wallet(&req.email, &req.wallet).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    info!("[Auth] Linked wallet {} to user {}", req.wallet, req.email);
    Ok(Json(()))
}

#[derive(Deserialize)]
pub struct WalletLoginReq {
    pub wallet: String,
    pub signature: String,
    pub timestamp: u64,
}

async fn login_wallet(
    State(state): State<AppState>,
    Json(req): Json<WalletLoginReq>,
) -> Result<Json<AuthResp>, (StatusCode, String)> {
    // 1. Verify signature to prove wallet ownership
    let pk = Pubkey::from_str(&req.wallet)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid wallet address".to_string()))?;
    let sig = Signature::from_str(&req.signature)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid signature format".to_string()))?;
    
    let msg = format!("login_wallet:{}", req.timestamp);
    if !sig.verify(pk.as_ref(), msg.as_bytes()) {
        return Err((StatusCode::UNAUTHORIZED, "Invalid cryptographic signature".to_string()));
    }

    // 2. Lookup existing account linked to this wallet
    let user = state.store.find_user_by_wallet(&req.wallet).await
        .ok_or((StatusCode::UNAUTHORIZED, "No XFChess account is linked to this wallet. Please register with email first.".to_string()))?;

    // 3. Issue JWT
    let token = state.jwt.issue(&user.0).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    info!("[Auth] Wallet-login successful for user: {}", user.0);

    Ok(Json(AuthResp {
        token,
        username: user.2,
        wallet: Some(req.wallet),
    }))
}
