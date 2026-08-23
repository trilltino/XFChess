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

use crate::signing::solana;
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
pub(crate) async fn authed_wallet(
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

/// Authenticates the caller via `authed_wallet`, then requires that identity
/// to exactly match `claimed_wallet` — the shared chokepoint for "this route
/// acts on behalf of exactly one wallet, no relaying, no exceptions" (KYC
/// submission, Lichess linking). This is a strictly narrower check than
/// `record_move`'s participant-aware relay logic (`routes::main::record_move`
/// — a game's host legitimately submits moves for BOTH players, so that one
/// stays a bespoke on-chain-participation check, not this). Before this
/// helper existed, `submit_kyc` and `init_oauth` each hand-rolled the same
/// six lines independently — exactly the kind of per-route reinvention that
/// let `record_move`'s equivalent check regress unnoticed earlier in this
/// project's history. New routes with this "caller == the one wallet this
/// acts on" shape should call this instead of rewriting it again.
pub(crate) async fn require_caller_owns_wallet(
    state: &AppState,
    headers: &axum::http::HeaderMap,
    claimed_wallet: &str,
) -> Result<String, (StatusCode, String)> {
    let wallet = authed_wallet(state, headers).await?;
    if wallet != claimed_wallet {
        return Err((
            StatusCode::FORBIDDEN,
            format!(
                "Authenticated wallet {wallet} does not match the wallet this request claims to act on ('{claimed_wallet}')"
            ),
        ));
    }
    Ok(wallet)
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
        .route(
            "/init-profile-sponsored-tx",
            post(init_profile_sponsored_tx),
        )
        .route("/broadcast-tx", post(broadcast_tx))
        .route("/username", axum::routing::patch(set_username))
        .route("/check-username/{username}", get(check_username))
        .route("/check-wallet/{wallet}", get(check_wallet))
        .route("/delete", post(delete_account))
        .route("/siws-challenge", post(siws_challenge))
        .route("/siws-verify", post(siws_verify))
        .route("/privy-login", post(privy_login))
}

// ── Privy social login ─────────────────────────────────────────────────────────

/// Maximum social identities created per hour across the instance before new
/// signups are refused. Social signup is ~free, so this is a cheap Sybil brake;
/// the real gate on wagering remains KYC + CACF (see `me`'s `can_wager`).
const SOCIAL_SIGNUP_HOURLY_CAP: i64 = 200;

/// POST /auth/privy-login — authenticate a Google/email user whose Solana
/// wallet was created by Privy.
///
/// Body: `{ privy_token, wallet, signature, timestamp, username? }`
///
/// # Why both a wallet signature and a Privy token
///
/// The **wallet signature is the authority**, exactly as for Phantom: it proves
/// control of the keypair that owns every on-chain asset and PDA. The Privy
/// token is corroborating evidence that binds a social credential to that
/// wallet, so we can (a) offer "sign in with Google" on the next visit and
/// (b) enforce one-account-per-email.
///
/// Consequence worth stating plainly: a stolen Privy token alone authenticates
/// nobody, because it cannot produce the signature. And a wallet signature alone
/// still works through the ordinary `/auth/login` route — this endpoint adds a
/// credential binding, it does not replace anything.
#[derive(Deserialize)]
struct PrivyLoginReq {
    privy_token: String,
    wallet: String,
    signature: String,
    timestamp: u64,
    /// Desired handle on first signup. Defaults to a pubkey prefix, matching
    /// the wallet-ui and website register paths.
    username: Option<String>,
    /// Email from the user's Privy linked accounts, when they have one.
    ///
    /// Not read from the access token: Privy's standard claims carry only the
    /// DID (`sub`), not contact details. This is therefore client-asserted, and
    /// is used ONLY for the D3 one-account-per-email guard and for display — it
    /// never authenticates anything. Spoofing it can at worst lock the spoofer
    /// out of their own signup by colliding with someone else's address; it
    /// cannot grant access to that other account, since the wallet signature
    /// still has to match.
    email: Option<String>,
    /// Which Privy method was used: `google`, `email`, … Display/analytics only.
    /// Constrained to a short allowlist below so an arbitrary client string
    /// never lands in the database.
    login_method: Option<String>,
}

/// Login methods we record. Anything else is stored as `unknown` rather than
/// trusted verbatim — this column is client-asserted.
const KNOWN_LOGIN_METHODS: &[&str] = &["google", "email", "apple", "discord", "github", "twitter"];

async fn privy_login(
    State(state): State<AppState>,
    Json(req): Json<PrivyLoginReq>,
) -> Result<Json<AuthResp>, (StatusCode, String)> {
    use crate::signing::privy;

    // Server-side half of the kill switch: with PRIVY_APP_ID unset this route
    // behaves as if it does not exist.
    if !privy::is_configured() {
        return Err((
            StatusCode::NOT_FOUND,
            "Social login is not enabled on this server".to_string(),
        ));
    }

    // 1. Wallet signature — the authority. Same helper, same replay window as
    //    every other wallet route; a Privy wallet is an ordinary ed25519 keypair.
    verify_wallet_sig(&req.wallet, &req.signature, "login", req.timestamp)?;

    // 2. Privy token — corroboration. Fails closed on an unreachable JWKS.
    let claims = privy::verify_access_token(&req.privy_token)
        .await
        .map_err(|e| match e {
            privy::PrivyError::Jwks(msg) => {
                tracing::error!("[Privy] JWKS unavailable, refusing login: {msg}");
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Could not verify social sign-in right now. Please try again.".to_string(),
                )
            }
            other => (StatusCode::UNAUTHORIZED, other.to_string()),
        })?;

    let provider = "privy";
    let subject = claims.sub.clone();
    // Absence is fine — the D3 email guard simply does not apply to an account
    // that has no email on file.
    let email_owned = req
        .email
        .as_ref()
        .map(|e| e.trim().to_lowercase())
        .filter(|e| !e.is_empty());
    let email = email_owned.as_deref();

    // 3. Existing binding for this credential?
    let bound = state.store.find_wallet_by_social(provider, &subject).await;

    if let Some(bound_wallet) = bound {
        // Refuse to re-point an existing credential at a different wallet. Same
        // invariant `link_wallet` enforces for email/password accounts: silently
        // re-attaching would detach this identity from whatever KYC, CACF and
        // session history sits under the old wallet, none of which is reconciled
        // anywhere.
        if bound_wallet != req.wallet {
            return Err((
                StatusCode::CONFLICT,
                "This social account is already linked to a different wallet and cannot be \
                 re-linked automatically. Contact support if you need to change it."
                    .to_string(),
            ));
        }
    } else {
        // 4. First binding for this credential. Enforce D3 on the email, if any.
        if let Some(addr) = email {
            if let Some(other) = state
                .store
                .find_wallet_by_social_email(provider, addr)
                .await
            {
                if other != req.wallet {
                    let handle = state
                        .store
                        .find_user_by_wallet(&other)
                        .await
                        .map(|u| u.1)
                        .unwrap_or_else(|| "an existing account".to_string());
                    return Err((
                        StatusCode::CONFLICT,
                        format!(
                            "This email is already linked to {handle}. Sign in with that wallet, \
                             then link this account from Settings."
                        ),
                    ));
                }
            }
        }

        let hour_ago = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
            - 3600;
        if state.store.count_social_identities_since(hour_ago).await >= SOCIAL_SIGNUP_HOURLY_CAP {
            tracing::warn!("[Privy] hourly social signup cap reached, refusing new binding");
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                "Too many new accounts right now. Please try again shortly.".to_string(),
            ));
        }
    }

    // 5. Ensure the XFChess account exists. A Privy user is a first-class wallet
    //    user — same table, same columns, no second tier (plan D2).
    let username = match state.store.find_user_by_wallet(&req.wallet).await {
        Some(user) => user.1,
        None => {
            let desired = req
                .username
                .filter(|u| !u.trim().is_empty())
                .unwrap_or_else(|| req.wallet.chars().take(8).collect());
            if state.store.username_taken(&desired).await {
                return Err((StatusCode::CONFLICT, "Username already taken".to_string()));
            }
            state
                .store
                .create_wallet_user(&req.wallet, &desired, email)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            desired
        }
    };

    // 6. Record/refresh the credential binding.
    let login_method = req
        .login_method
        .as_deref()
        .map(str::trim)
        .filter(|m| KNOWN_LOGIN_METHODS.contains(m))
        .unwrap_or("unknown");
    state
        .store
        .upsert_social_identity(provider, &subject, &req.wallet, login_method, email, true)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 7. Ordinary XFChess JWT, keyed on the wallet like every other login.
    let token = state
        .jwt
        .issue(&req.wallet)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    info!(
        "[Auth] Privy login: wallet={} subject={}",
        req.wallet, subject
    );
    Ok(Json(AuthResp {
        token,
        username,
        wallet: req.wallet,
    }))
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

    // 3. Refuse to silently re-point an already-linked account to a
    // different wallet. `link_wallet`'s UPDATE has no concept of "this
    // account already has an identity" — letting it succeed here would
    // silently detach this email/username from its current wallet (and
    // whatever KYC/CACF/session history sits under that wallet string,
    // none of which is reconciled anywhere) and re-attach the account to a
    // new one, with no history or explicit consent step. Linking a wallet
    // for the first time (the common case: an email-first account that
    // never had one) is unaffected — this only blocks a SECOND link to a
    // DIFFERENT wallet.
    if !user.0.is_empty() && user.0 != req.wallet {
        return Err((
            StatusCode::CONFLICT,
            "This account is already linked to a different wallet and cannot be re-linked \
             automatically. Contact support if you need to change the linked wallet."
                .to_string(),
        ));
    }

    // 4. Link Wallet
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
    /// Lichess blitz rating (0 = not linked / no games). Shown as a second,
    /// clearly-labeled stat alongside `elo` — never merged.
    lichess_blitz: u32,
    lichess_verified: bool,
    /// Social providers bound to this wallet, e.g. `["google"]`. Empty for a
    /// wallet-only (Phantom/Solflare) account.
    login_methods: Vec<String>,
    /// True when this wallet is a Privy-created embedded wallet.
    ///
    /// **UI only** — it drives "back up your wallet" nudges and the
    /// un-backed-up balance cap. It must never influence `can_wager`: a social
    /// user is a first-class wallet user (plan D2).
    is_embedded_wallet: bool,
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
    let vault = crate::signing::storage::vault::VaultStore::new(
        (*state.vault_pool).clone(),
        state.store.pool(),
    );
    let has_kyc = vault.has_kyc(&user.0).await;
    let kyc_country = vault.get_kyc(&user.0).await.map(|r| r.country);
    let cacf_ok = match &kyc_country {
        Some(c) => vault.cacf_can_wager(&user.0, c).await,
        None => true,
    };
    // Devnet bypass mirrors kyc.rs::user_status — see is_devnet() doc comment.
    let can_wager = state.config.is_devnet() || (wallet_linked && has_kyc && cacf_ok);

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
    // `elo_rating` is stored centiscale (1200 Elo = 120000) — convert to
    // display scale, matching `external_elo.rs`/`anticheat_enqueue.rs`.
    let cached_elo = state.elo_cache.get_elo(&wallet).await.ok();
    let elo = cached_elo
        .as_ref()
        .map(|e| (e.elo_rating / 100.0).round() as u32)
        .unwrap_or(0);
    let lichess_blitz = cached_elo.as_ref().map(|e| e.lichess_blitz).unwrap_or(0);
    let lichess_verified = cached_elo
        .as_ref()
        .map(|e| e.lichess_verified)
        .unwrap_or(false);
    let country = kyc_country.unwrap_or_default();

    // Social-credential metadata. Presentational only — `can_wager` above is
    // deliberately computed without reference to either of these.
    let login_methods = state.store.social_login_methods(&wallet).await;
    let is_embedded_wallet = state.store.wallet_is_embedded(&wallet).await;

    Ok(Json(MeResp {
        wallet: user.0,
        username: user.1,
        email: user.2,
        kyc_status: user.3,
        wallet_linked,
        can_wager,
        has_onchain_profile,
        lichess_blitz,
        lichess_verified,
        elo,
        country,
        login_methods,
        is_embedded_wallet,
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
/// Field order and widths MUST match
/// `programs/xfchess-game/src/state/player_profile.rs` exactly up to the last
/// field declared here — borsh is positional, so one wrong width silently
/// shifts everything after it.
///
/// Decoded leniently (see `fetch_onchain_profile`), so declaring *fewer*
/// trailing fields than the on-chain struct is safe: the tail is just left
/// unread alongside Anchor's `#[max_len]` padding. Declaring more is also
/// tolerated as long as the account still has padding to read them out of —
/// a deployed program lagging behind this file (devnet accounts are 265 bytes,
/// i.e. pre-`elo_bullet`) yields `0.0`/zero for the extra fields rather than
/// an error. Don't read a field here that the deployed program may not have.
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
    pub _date_of_birth: i64,
    pub _is_verified: bool,
    pub _annual_wins_gbp: u64,
    pub _annual_wins_brl: u64,
    pub _annual_wins_cad: u64,
    pub _annual_wins_eur: u64,
    pub username: String,
    pub username_set: bool,
    pub _lichess_username: String,
    pub _lichess_verified: bool,
    pub _lichess_blitz: u32,
    pub _lichess_rapid: u32,
    pub _lichess_bullet: u32,
    pub _lichess_last_sync: i64,
    pub _external_elo_source: u8,
    pub _seeded_from_external: bool,
    pub _elo_bullet: f64,
    pub _elo_blitz: f64,
    pub _elo_rapid: f64,
}

/// Fetches and borsh-decodes the caller's on-chain `PlayerProfile`, or
/// `None` if the account doesn't exist yet — a normal state for a wallet
/// that hasn't wagered/initialized on-chain yet, not an error. Shared by
/// `sync_profile` and `set_username`'s on-chain-precedence guard (see the
/// latter's doc comment for why both need the same read).
async fn fetch_onchain_profile(
    state: &AppState,
    wallet: &str,
) -> Result<Option<ProfileOnChain>, (StatusCode, String)> {
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

    let rpc =
        solana_client::nonblocking::rpc_client::RpcClient::new(state.config.solana_rpc_url.clone());
    let account = match rpc.get_account(&profile_pda).await {
        Ok(account) => account,
        Err(_) => return Ok(None),
    };

    if account.data.len() < 9 {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "Account data too short".to_string(),
        ));
    }
    // `deserialize`, not `try_from_slice`: Anchor allocates `8 +
    // PlayerProfile::INIT_SPACE`, which reserves the *max* length of every
    // `#[max_len]` string, while borsh writes only the actual bytes. Every
    // real profile therefore ends in zero padding, and `try_from_slice`
    // rejects leftover bytes ("Not all bytes read") — which made this fail
    // for every wallet that had a profile at all. Reading only the fields we
    // declare and ignoring the tail is exactly what Anchor's own
    // `try_deserialize` does on-chain.
    let profile = ProfileOnChain::deserialize(&mut &account.data[8..]).map_err(|e| {
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            // Length is the first thing to check when this fires: a profile
            // created by an older program version is too short for the fields
            // declared above and hits EOF here (devnet still has 200-byte
            // pre-`date_of_birth` accounts from an old bot-seeding run).
            format!(
                "Failed to decode profile ({} bytes): {e}",
                account.data.len()
            ),
        )
    })?;
    Ok(Some(profile))
}

/// POST /auth/sync-profile — reads the caller's on-chain PlayerProfile PDA
/// and returns its status. This is the single source of truth wallet-ui and
/// the game client should both route on: whether an on-chain profile exists
/// at all, whether a username has been chosen, and whether the account is
/// KYC-verified (`is_verified`). "No profile yet" / "no username yet" are
/// normal states for a new wallet, not errors — this always returns 200
/// (barring a genuine auth/RPC failure) so callers can branch on the fields
/// instead of on HTTP status. Requires a valid Bearer JWT. Safe to retry —
/// idempotent; also mirrors the username into SQLite as a side effect once
/// one is set on-chain.
async fn sync_profile(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let wallet = authed_wallet(&state, &headers).await?;
    let wallet = &wallet;

    let Some(profile) = fetch_onchain_profile(&state, wallet).await? else {
        return Ok(Json(serde_json::json!({
            "has_profile": false,
            "username_set": false,
            "is_verified": false,
            "username": null,
        })));
    };

    // If a username is set, mirror it into SQLite as the canonical value.
    if profile.username_set && !profile.username.is_empty() {
        state
            .store
            .update_username(wallet, &profile.username)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        info!(
            "[Auth] Synced on-chain username '{}' for {}",
            profile.username, wallet
        );
    }

    Ok(Json(serde_json::json!({
        "has_profile": true,
        "username_set": profile.username_set && !profile.username.is_empty(),
        "is_verified": profile._is_verified,
        "username": if profile.username_set && !profile.username.is_empty() {
            Some(profile.username)
        } else {
            None
        },
    })))
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
        transaction::Transaction,
    };
    use solana_system_interface::program as system_program;

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
    // `finalized`, not `confirmed` — this transaction goes to a browser wallet
    // extension, which decides whether to sign it at all by looking the
    // blockhash up on its selected cluster at finalized commitment. See
    // `solana::wallet_signable_blockhash`.
    let rpc = std::sync::Arc::clone(&state.solana_rpc);
    let recent_blockhash =
        tokio::task::spawn_blocking(move || solana::wallet_signable_blockhash(&rpc))
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

// ── POST /auth/init-profile-sponsored-tx ──────────────────────────────────────

#[derive(Serialize)]
struct InitProfileSponsoredResp {
    tx_b64: String,
    profile_pda: String,
}

/// POST /auth/init-profile-sponsored-tx
///
/// Same as `init_profile_tx`, except XFChess pays the on-chain rent for the
/// player's *first* profile — removes the "need SOL before you can go
/// on-chain at all" problem. Gated on: (1) KYC submitted, (2) never
/// sponsored before for this account.
///
/// `InitProfile`'s `create_account` CPIs debit `player`, not whichever
/// account happens to be the transaction's fee payer — so merely paying the
/// tx fee wouldn't actually cover the meaningful cost (account rent). This
/// prepends a `system_instruction::transfer` from the backend fee payer to
/// the player for exactly the rent both PDAs need, so the existing,
/// unmodified `init_profile` instruction can then debit *that* balance from
/// the player as it always does — no program change required. Returns a
/// transaction partially signed by the backend (as fee payer); the player
/// still signs before broadcasting via `/auth/broadcast-tx`.
async fn init_profile_sponsored_tx(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<InitProfileTxReq>,
) -> Result<Json<InitProfileSponsoredResp>, (StatusCode, String)> {
    use base64::engine::general_purpose;
    use base64::Engine as _;
    use solana_sdk::{
        instruction::{AccountMeta, Instruction},
        message::Message,
        signature::Signer as _,
        transaction::Transaction,
    };
    use solana_system_interface::{instruction as system_instruction, program as system_program};

    let wallet = authed_wallet(&state, &headers).await?;
    let wallet_pk = Pubkey::from_str(&wallet).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid wallet in token".to_string(),
        )
    })?;

    // ── KYC gate ──────────────────────────────────────────────────────────
    // Uses the working KYC store (kyc_records, written by /api/kyc/submit) —
    // NOT vault_users, which historically was never populated. See
    // docs/plans/identity-implementation-plan.md.
    // Bypassed on devnet — mirrors the can_wager devnet bypass in kyc.rs/auth.rs
    // (see SigningConfig::is_devnet() doc comment); mainnet stays fully gated.
    let vault = crate::signing::storage::vault::VaultStore::new(
        (*state.vault_pool).clone(),
        state.store.pool(),
    );
    if !state.config.is_devnet() && !vault.has_kyc(&wallet).await {
        return Err((
            StatusCode::FORBIDDEN,
            "KYC verification required before creating an on-chain profile. Submit KYC via /api/kyc/submit first.".to_string(),
        ));
    }

    // ── One sponsorship per account ─────────────────────────────────────
    if state.store.profile_sponsored_at(&wallet).await.is_some() {
        return Err((
            StatusCode::CONFLICT,
            "This account has already received a sponsored profile creation.".to_string(),
        ));
    }

    // Validate inputs (same rules as init_profile_tx)
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
    let (profile_pda, _) =
        Pubkey::find_program_address(&[b"profile", wallet_pk.as_ref()], &program_id);
    let (username_record_pda, _) =
        Pubkey::find_program_address(&[b"username", req.username.as_bytes()], &program_id);

    let discriminator: [u8; 8] = [0xd2, 0xa2, 0xd4, 0x5f, 0x5f, 0xba, 0x59, 0x77];
    let mut data = Vec::with_capacity(64);
    data.extend_from_slice(&discriminator);
    let un_bytes = req.username.as_bytes();
    data.extend_from_slice(&(un_bytes.len() as u32).to_le_bytes());
    data.extend_from_slice(un_bytes);
    let co_bytes = req.country.as_bytes();
    data.extend_from_slice(&(co_bytes.len() as u32).to_le_bytes());
    data.extend_from_slice(co_bytes);
    data.extend_from_slice(&req.date_of_birth.to_le_bytes());

    let accounts = vec![
        AccountMeta::new(profile_pda, false),
        AccountMeta::new(username_record_pda, false),
        AccountMeta::new(wallet_pk, true),
        AccountMeta::new_readonly(system_program::id(), false),
    ];
    let init_ix = Instruction {
        program_id,
        accounts,
        data,
    };

    // discriminator(8) + PlayerProfile::INIT_SPACE(281: 257 plus the 24 bytes
    // added by `elo_bullet`/`elo_blitz`/`elo_rapid`, three f64 per-time-
    // control ratings appended to the struct). The username_record
    // account struct constraint is `space = 8 + UsernameRecord::LEN`, and
    // UsernameRecord::LEN (48) already includes its own discriminator — so
    // the real allocated space is 56, not 48 (verified against a live
    // ProgramTest run in programs/xfchess-game/tests/init_profile_sponsored_tests.rs,
    // which failed on-chain with "insufficient lamports" before this fix).
    // Kept in sync manually since the backend doesn't depend on the program
    // crate — see programs/xfchess-game/src/state/{player_profile.rs,username_record.rs}.
    const PROFILE_SPACE: usize = 8 + 281;
    const USERNAME_RECORD_SPACE: usize = 8 + 48;

    let rpc = std::sync::Arc::clone(&state.solana_rpc);
    let (rent_lamports, recent_blockhash) = tokio::task::spawn_blocking(move || {
        let profile_rent = rpc
            .get_minimum_balance_for_rent_exemption(PROFILE_SPACE)
            .map_err(|e| e.to_string())?;
        let username_rent = rpc
            .get_minimum_balance_for_rent_exemption(USERNAME_RECORD_SPACE)
            .map_err(|e| e.to_string())?;
        // Finalized: the player's wallet extension has to recognise this as a
        // devnet blockhash before it will sign — see `wallet_signable_blockhash`.
        let blockhash = solana::wallet_signable_blockhash(&rpc).map_err(|e| e.to_string())?;
        Ok::<_, String>((profile_rent + username_rent, blockhash))
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::BAD_GATEWAY, format!("RPC error: {e}")))?;

    let fee_payer = state.feepayer.next();
    let transfer_ix = system_instruction::transfer(&fee_payer.pubkey(), &wallet_pk, rent_lamports);

    let message = Message::new(&[transfer_ix, init_ix], Some(&fee_payer.pubkey()));
    let mut transaction = Transaction::new_unsigned(message);
    transaction
        .try_partial_sign(&[fee_payer], recent_blockhash)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("partial sign: {e}"),
            )
        })?;

    let tx_bytes = bincode::serialize(&transaction)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("serialize: {e}")))?;
    let tx_b64 = general_purpose::STANDARD.encode(&tx_bytes);

    let now = chrono::Utc::now().timestamp();
    if let Err(e) = state.store.mark_profile_sponsored(&wallet, now).await {
        tracing::warn!(
            "[Auth] Failed to record profile sponsorship for {}: {}",
            wallet,
            e
        );
    }

    info!(
        "[Auth] Built sponsored init_profile_tx for {} username={} rent_lamports={}",
        wallet_pk, req.username, rent_lamports
    );
    Ok(Json(InitProfileSponsoredResp {
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
    use solana_commitment_config::CommitmentConfig;
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

/// PATCH /auth/username — updates the display username in SQLite for the
/// JWT's wallet. Checks availability then writes. Does not touch the
/// on-chain account, which is exactly the problem once one exists:
/// `resolveExistingUsername` (wallet-ui) and this web app's `ProfileStep`
/// both prefer the on-chain `PlayerProfile.username` once `username_set` is
/// true, so an off-chain-only rename at that point would silently apply to
/// a field neither surface ever displays again — the player sees "success"
/// here while nothing anywhere actually shows the new name. Once the
/// on-chain username is set, a rename must go through the on-chain path
/// (re-submit `init_profile`/`init-profile-sponsored-tx` with the new name,
/// which the player signs) instead. Before that point — the common case,
/// since on-chain profile init is deferred to the player's first wager —
/// this off-chain path is the only one that exists and works as before.
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

    if let Some(onchain) = fetch_onchain_profile(&state, &wallet).await? {
        if onchain.username_set && !onchain.username.is_empty() {
            return Err((
                StatusCode::CONFLICT,
                "On-chain username is already set — update it via the on-chain profile flow \
                 (re-submit init_profile with the new name) instead of this off-chain-only route."
                    .to_string(),
            ));
        }
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
    let vault = crate::signing::storage::vault::VaultStore::new(
        (*state.vault_pool).clone(),
        state.store.pool(),
    );
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

#[cfg(test)]
mod profile_decode_tests {
    use super::ProfileOnChain;
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine as _;
    use borsh::BorshDeserialize;

    /// A real devnet `PlayerProfile` account (PDA
    /// `2rnL1R63FndwkN8UcybiiwVqMCuR2vTirGZU48NH7bzT`, username "val"),
    /// captured verbatim via `getAccountInfo`. Anchor allocates
    /// `8 + PlayerProfile::INIT_SPACE` — the *max* length of every
    /// `#[max_len]` string — but borsh writes only the actual string bytes,
    /// so every real account carries trailing zero padding.
    const REAL_PROFILE_B64: &str = "UuJjV6SCtVAsepYl9NwToT6BbL/CZ8gRg2700PZfdsZA9JatrWSSmQIAAABHQgIAAAABAAAAAAAAAAMAAAAAAAAAAEz9QAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAlriBagAAAAAAAAAAAAAAAIDUTj4AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAwAAAHZhbAEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==";

    /// Regression: `try_from_slice` rejects *any* leftover bytes, so decoding
    /// a real profile with it fails with "Not all bytes read" — which
    /// `sync_profile` surfaced as a blanket 422 for every wallet that had an
    /// on-chain profile. Anchor's own `try_deserialize` reads the fields it
    /// needs and ignores the padding; the backend mirror must do the same.
    #[test]
    fn decodes_a_real_padded_devnet_profile() {
        let data = STANDARD.decode(REAL_PROFILE_B64).unwrap();
        let mut rest = &data[8..];
        let profile = ProfileOnChain::deserialize(&mut rest)
            .expect("real on-chain profile must decode past its zero padding");

        assert_eq!(profile.username, "val");
        assert!(profile.username_set);
        assert!(
            !rest.is_empty(),
            "fixture should still have trailing padding — otherwise it isn't \
             exercising the leftover-bytes case"
        );
        assert!(
            ProfileOnChain::try_from_slice(&data[8..]).is_err(),
            "strict decoding must still reject the padding — if this ever \
             passes, Anchor stopped over-allocating and the guard is moot"
        );
    }
}
