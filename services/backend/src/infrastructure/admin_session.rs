//! Username and password sign-in for the admin panel and the admin CLIs.
//!
//! `POST /admin/auth/login {username, password}` returns a session token. Every admin route takes
//! it in the [`SESSION_HEADER`] header (`auth_middleware::require_admin`); there is no other admin
//! credential.
//!
//! - `ADMIN_USERS` lists the accounts as `name:HASH` entries separated by `;`, where HASH is an
//!   argon2 PHC string. `cargo run -p backend --bin admin_password -- <name>` asks for a password
//!   and prints the line. Keep the value in single quotes in `.env`, because the hash contains `$`.
//!   Unset or empty means nobody can sign in.
//! - A session is an HS256 JWT valid for eight hours, signed with a key derived from `JWT_SECRET`
//!   that player tokens do not share, so a player token is never an admin session.
//! - Every request checks the account again: removing it from `ADMIN_USERS` or changing its
//!   password ends its sessions at once.
//! - Five wrong passwords in a row lock an account for five minutes, and at most two password
//!   checks run at a time. `/admin` is not served publicly (nginx), so callers come through the
//!   SSH tunnel or the machine itself.

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{http::StatusCode, routing::post, Json, Router};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::env;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// The request header that carries an admin session token.
pub const SESSION_HEADER: &str = "x-admin-session";
const SESSION_TTL_SECS: u64 = 8 * 60 * 60;
const SESSION_KIND: &str = "admin-session";
const MAX_FAILURES: u32 = 5;
const LOCKOUT_SECS: u64 = 5 * 60;
const MAX_TRACKED_NAMES: usize = 1024;
const MAX_NAME_LEN: usize = 32;
const MAX_PASSWORD_LEN: usize = 1024;
const WRONG_CREDENTIALS: &str = "wrong username or password";

static PASSWORD_WORK: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

struct AdminUser {
    name: String,
    hash: String,
}

/// An admin name: 1-32 ASCII letters, digits, `_`, `-` or `.`.
pub fn valid_name(name: &str) -> bool {
    (1..=MAX_NAME_LEN).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

/// Each `;`-separated entry of an `ADMIN_USERS` value, or why it is not a usable account.
fn parse_users(raw: &str) -> Vec<Result<AdminUser, String>> {
    raw.trim()
        .trim_matches(|c| c == '\'' || c == '"')
        .split(';')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            let (name, hash) = entry
                .split_once(':')
                .ok_or_else(|| "an ADMIN_USERS entry is not name:hash".to_string())?;
            let (name, hash) = (name.trim(), hash.trim());
            if !valid_name(name) {
                return Err(format!(
                    "ADMIN_USERS name {name:?} must be 1-{MAX_NAME_LEN} letters, digits, '_', '-' or '.'"
                ));
            }
            let parsed = PasswordHash::new(hash)
                .map_err(|_| format!("the ADMIN_USERS hash for {name} is not a PHC string"))?;
            if !parsed.algorithm.as_str().starts_with("argon2") {
                return Err(format!("the ADMIN_USERS hash for {name} is not an argon2 hash"));
            }
            Ok(AdminUser {
                name: name.to_string(),
                hash: hash.to_string(),
            })
        })
        .collect()
}

fn admin_users() -> Vec<AdminUser> {
    let raw = env::var("ADMIN_USERS").unwrap_or_default();
    parse_users(&raw).into_iter().filter_map(Result::ok).collect()
}

/// Why nobody could sign in with the current `ADMIN_USERS`, if that is the case or an entry is
/// unusable. Startup validation reports it (fatal in production).
pub fn configuration_problem() -> Option<String> {
    let raw = env::var("ADMIN_USERS").unwrap_or_default();
    let entries = parse_users(&raw);
    if entries.is_empty() {
        return Some(
            "ADMIN_USERS not set — nobody can sign in to the admin panel \
             (cargo run -p backend --bin admin_password -- <name>)"
                .to_string(),
        );
    }
    entries.into_iter().find_map(Result::err)
}

/// An argon2id PHC string for `password`, in the form `ADMIN_USERS` stores.
pub fn hash_password(password: &str) -> Result<String, String> {
    Argon2::default()
        .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
        .map(|hash| hash.to_string())
        .map_err(|e| e.to_string())
}

/// Checked when the name is unknown, so the time taken does not tell which names exist.
fn decoy_hash() -> &'static str {
    static DECOY: OnceLock<String> = OnceLock::new();
    DECOY.get_or_init(|| hash_password(&uuid::Uuid::new_v4().to_string()).unwrap_or_default())
}

async fn password_matches(
    password: String,
    hash: Option<String>,
) -> Result<bool, (StatusCode, &'static str)> {
    let permit = PASSWORD_WORK
        .acquire()
        .await
        .map_err(|_| (StatusCode::SERVICE_UNAVAILABLE, "sign-in is unavailable"))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let hash = hash.unwrap_or_else(|| decoy_hash().to_string());
        PasswordHash::new(&hash).is_ok_and(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
    })
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "sign-in failed"))
}

/// Ties a session to the password it was issued for: a new hash in `ADMIN_USERS` ends it.
fn fingerprint(hash: &str) -> String {
    hex::encode(&Sha256::digest(hash.as_bytes())[..8])
}

fn session_secret() -> Option<String> {
    let secret = env::var("JWT_SECRET").ok().filter(|s| !s.is_empty())?;
    Some(format!("xfchess-admin-session|{secret}"))
}

#[derive(Serialize, Deserialize)]
struct SessionClaims {
    sub: String,
    kind: String,
    pw: String,
    iat: u64,
    exp: u64,
}

fn issue_session(user: &AdminUser) -> Option<(String, u64)> {
    let secret = session_secret()?;
    let issued = now();
    let expires = issued + SESSION_TTL_SECS;
    let claims = SessionClaims {
        sub: user.name.clone(),
        kind: SESSION_KIND.to_string(),
        pw: fingerprint(&user.hash),
        iat: issued,
        exp: expires,
    };
    let token = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .ok()?;
    Some((token, expires))
}

/// The admin name for a valid session token. The account is checked against the current
/// `ADMIN_USERS`, so removing it or changing its password ends the session at the next request.
pub fn actor_for_session(token: &str) -> Result<String, StatusCode> {
    if token.is_empty() {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let secret = session_secret().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let mut validation = Validation::new(Algorithm::HS256);
    validation.leeway = 0;
    validation.set_required_spec_claims(&["exp", "sub"]);
    let claims = decode::<SessionClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|_| StatusCode::UNAUTHORIZED)?
    .claims;
    if claims.kind != SESSION_KIND {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let user = admin_users()
        .into_iter()
        .find(|user| user.name == claims.sub)
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if fingerprint(&user.hash) != claims.pw {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(user.name)
}

// ── Wrong-password lockout ────────────────────────────────────────────────────────────────────

struct Failures {
    count: u32,
    last: u64,
}

type FailureMap = HashMap<String, Failures>;

fn failure_map() -> &'static Mutex<FailureMap> {
    static FAILURES: OnceLock<Mutex<FailureMap>> = OnceLock::new();
    FAILURES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Whether `name` must wait before another try. Also true when so many names are failing that no
/// new one can be tracked, so filling the table cannot switch the lockout off.
fn locked_out(map: &mut FailureMap, name: &str, now: u64) -> bool {
    map.retain(|_, f| now < f.last + LOCKOUT_SECS);
    match map.get(name) {
        Some(f) => f.count >= MAX_FAILURES,
        None => map.len() >= MAX_TRACKED_NAMES,
    }
}

fn record_failure(map: &mut FailureMap, name: &str, now: u64) {
    let entry = map
        .entry(name.to_string())
        .or_insert(Failures { count: 0, last: now });
    if now >= entry.last + LOCKOUT_SECS {
        entry.count = 0;
    }
    entry.count += 1;
    entry.last = now;
}

// ── Route ─────────────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
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
    let users = admin_users();
    if users.is_empty() || session_secret().is_none() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "admin sign-in is not configured: set ADMIN_USERS and JWT_SECRET",
        ));
    }
    let username = req.username.trim().to_string();
    if !valid_name(&username) || req.password.is_empty() || req.password.len() > MAX_PASSWORD_LEN {
        return Err((StatusCode::UNAUTHORIZED, WRONG_CREDENTIALS));
    }
    if locked_out(&mut failure_map().lock(), &username, now()) {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "too many wrong passwords; wait five minutes and try again",
        ));
    }
    let user = users.into_iter().find(|user| user.name == username);
    let matches = password_matches(req.password, user.as_ref().map(|u| u.hash.clone())).await?;
    let Some(user) = user.filter(|_| matches) else {
        record_failure(&mut failure_map().lock(), &username, now());
        tracing::warn!("[auth] admin sign-in refused for {username:?}");
        return Err((StatusCode::UNAUTHORIZED, WRONG_CREDENTIALS));
    };
    failure_map().lock().remove(&username);
    let (token, expires_at) = issue_session(&user).ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "admin sign-in is not configured: set JWT_SECRET",
    ))?;
    tracing::info!("[auth] admin sign-in: {}", user.name);
    Ok(Json(LoginResponse {
        token,
        actor: user.name,
        expires_at,
    }))
}

/// Mounted outside the admin guard: this is how a caller obtains a session.
pub fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new().route("/login", post(login))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_entries_and_names_what_is_wrong() -> Result<(), String> {
        let hash = hash_password("pw")?;
        let raw = format!("'tino:{hash}; ops-2:{hash};'");
        let users: Vec<_> = parse_users(&raw).into_iter().collect::<Result<_, _>>()?;
        assert_eq!(
            users.iter().map(|u| u.name.as_str()).collect::<Vec<_>>(),
            ["tino", "ops-2"]
        );
        assert_eq!(users[0].hash, hash);

        let errors = |raw: &str| {
            parse_users(raw)
                .into_iter()
                .filter_map(Result::err)
                .collect::<Vec<_>>()
        };
        assert_eq!(errors("no-colon").len(), 1);
        assert_eq!(errors("tino:plaintext-password").len(), 1);
        assert_eq!(errors(&format!("bad name:{hash}")).len(), 1);
        assert_eq!(errors(&format!(":{hash}")).len(), 1);
        assert!(parse_users("  ").is_empty());
        Ok(())
    }

    #[test]
    fn a_new_hash_changes_the_fingerprint() -> Result<(), String> {
        let first = hash_password("same password")?;
        let second = hash_password("same password")?;
        assert_ne!(fingerprint(&first), fingerprint(&second));
        assert_eq!(fingerprint(&first), fingerprint(&first));
        Ok(())
    }

    #[test]
    fn five_wrong_passwords_lock_an_account_for_five_minutes() {
        let mut map = FailureMap::new();
        for _ in 0..MAX_FAILURES {
            assert!(!locked_out(&mut map, "tino", 1_000));
            record_failure(&mut map, "tino", 1_000);
        }
        assert!(locked_out(&mut map, "tino", 1_000));
        assert!(!locked_out(&mut map, "someone-else", 1_000));
        assert!(locked_out(&mut map, "tino", 1_000 + LOCKOUT_SECS - 1));
        assert!(!locked_out(&mut map, "tino", 1_000 + LOCKOUT_SECS));
    }

    #[test]
    fn a_full_failure_table_locks_new_names_instead_of_forgetting() {
        let mut map = FailureMap::new();
        for i in 0..MAX_TRACKED_NAMES {
            record_failure(&mut map, &format!("name-{i}"), 1_000);
        }
        assert!(locked_out(&mut map, "tino", 1_000));
        assert!(!locked_out(&mut map, "tino", 1_000 + LOCKOUT_SECS));
    }
}
