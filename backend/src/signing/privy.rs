//! Privy access-token verification.
//!
//! Privy issues **ES256** JWTs signed with keys published at a public,
//! unauthenticated JWKS endpoint. Verifying them therefore needs no Privy SDK
//! and no app secret — `jsonwebtoken` (already a dependency) handles ES256, and
//! the JWKS is a plain GET.
//!
//! ```text
//! GET https://auth.privy.io/api/v1/apps/{app_id}/jwks.json
//! ```
//!
//! # Design notes
//!
//! - **`kid` selection is mandatory.** The live endpoint returns *two* EC P-256
//!   keys (verified 2026-08-23), because Privy rotates. Taking `keys[0]` would
//!   verify correctly right up until the day it silently didn't.
//! - **Fail closed.** If the JWKS cannot be fetched or the `kid` is unknown,
//!   verification fails. There is no unverified fallback path, by design: the
//!   token is what proves the caller controls the social account.
//! - **`aud` is a set, not a value.** One app ID today, but the plan's §10
//!   splits web and desktop into separate Privy apps at the mainnet cutover.
//!   Accepting a set from the start makes that a config change instead of a code
//!   change.
//! - This token is **corroborating evidence, never the sole authority**. Every
//!   route that consumes it also requires a wallet signature over
//!   `xfchess:<action>:<ts>` (see `verify_wallet_sig`), so a stolen Privy token
//!   on its own cannot authenticate anyone.
//!
//! See docs/plans/social-login-embedded-wallet-plan.md §9.2.

use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// How long a fetched JWKS is reused before being refetched.
const JWKS_CACHE_TTL: Duration = Duration::from_secs(3600);

/// Guards against a pathological/hostile JWKS response.
const JWKS_MAX_KEYS: usize = 16;

/// Privy's token issuer claim — constant across all apps.
const PRIVY_ISSUER: &str = "privy.io";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Jwk {
    pub kty: String,
    pub crv: String,
    pub x: String,
    pub y: String,
    pub kid: String,
    #[serde(default)]
    pub alg: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Jwks {
    pub keys: Vec<Jwk>,
}

/// Claims XFChess cares about from a Privy access token.
#[derive(Debug, Clone, Deserialize)]
pub struct PrivyClaims {
    /// Privy DID, e.g. `did:privy:clx…`. This is the credential subject.
    pub sub: String,
    /// Always `privy.io`.
    pub iss: String,
    /// The Privy app ID this token was minted for.
    pub aud: String,
    pub exp: i64,
    #[serde(default)]
    pub iat: i64,
    /// Privy session id.
    #[serde(default)]
    pub sid: Option<String>,
}

#[derive(Debug)]
pub enum PrivyError {
    /// JWKS could not be fetched or parsed. Fail closed.
    Jwks(String),
    /// Token header/format is unusable.
    Malformed(String),
    /// Signature, `exp`, `iss` or `aud` did not check out.
    Invalid(String),
    /// Privy is not configured on this deployment.
    NotConfigured,
}

impl std::fmt::Display for PrivyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Jwks(e) => write!(f, "Privy key set unavailable: {e}"),
            Self::Malformed(e) => write!(f, "Malformed Privy token: {e}"),
            Self::Invalid(e) => write!(f, "Invalid Privy token: {e}"),
            Self::NotConfigured => write!(f, "Privy social login is not enabled on this server"),
        }
    }
}

/// Splits the configured `PRIVY_APP_ID` value into accepted `aud` values.
///
/// Pure so it is testable without mutating process env — these tests otherwise
/// race each other, since Rust runs them in parallel threads of one process.
fn parse_app_ids(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Comma-separated list of accepted Privy app IDs (the `aud` claim).
pub fn app_ids() -> Vec<String> {
    parse_app_ids(&std::env::var("PRIVY_APP_ID").unwrap_or_default())
}

/// Whether social login is configured. When false every Privy route should
/// behave as if it does not exist — this is the server-side half of the
/// `VITE_PRIVY_APP_ID` kill switch.
pub fn is_configured() -> bool {
    !app_ids().is_empty()
}

/// JWKS URL for an app, given an optional explicit override. Pure — see
/// `parse_app_ids` for why.
fn jwks_url_for(app_id: &str, override_url: Option<&str>) -> String {
    match override_url.map(str::trim).filter(|s| !s.is_empty()) {
        Some(url) => url.to_string(),
        None => format!("https://auth.privy.io/api/v1/apps/{app_id}/jwks.json"),
    }
}

/// JWKS URL for an app. Overridable via `PRIVY_JWKS_URL` (single-app setups and
/// tests); otherwise derived from the app ID.
fn jwks_url(app_id: &str) -> String {
    jwks_url_for(app_id, std::env::var("PRIVY_JWKS_URL").ok().as_deref())
}

type CacheCell = Mutex<Option<(Jwks, Instant)>>;

fn cache() -> &'static CacheCell {
    static CELL: OnceLock<CacheCell> = OnceLock::new();
    CELL.get_or_init(|| Mutex::new(None))
}

async fn fetch_jwks(app_id: &str) -> Result<Jwks, PrivyError> {
    let url = jwks_url(app_id);
    let resp = reqwest::Client::new()
        .get(&url)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| PrivyError::Jwks(format!("GET {url}: {e}")))?;

    if !resp.status().is_success() {
        return Err(PrivyError::Jwks(format!(
            "GET {url}: HTTP {}",
            resp.status()
        )));
    }

    let jwks: Jwks = resp
        .json()
        .await
        .map_err(|e| PrivyError::Jwks(format!("parse {url}: {e}")))?;

    if jwks.keys.is_empty() {
        return Err(PrivyError::Jwks("key set is empty".into()));
    }
    if jwks.keys.len() > JWKS_MAX_KEYS {
        return Err(PrivyError::Jwks(format!(
            "key set has {} keys, refusing (max {JWKS_MAX_KEYS})",
            jwks.keys.len()
        )));
    }
    Ok(jwks)
}

/// Returns a cached JWKS, refetching when stale. `force_refresh` bypasses the
/// cache — used exactly once per verification when a `kid` is unknown, so a key
/// rotation resolves on the next request instead of after a full TTL of 401s.
async fn get_jwks(app_id: &str, force_refresh: bool) -> Result<Jwks, PrivyError> {
    let mut guard = cache().lock().await;

    if !force_refresh {
        if let Some((jwks, fetched_at)) = guard.as_ref() {
            if fetched_at.elapsed() < JWKS_CACHE_TTL {
                return Ok(jwks.clone());
            }
        }
    }

    let jwks = fetch_jwks(app_id).await?;
    *guard = Some((jwks.clone(), Instant::now()));
    Ok(jwks)
}

fn decoding_key(jwk: &Jwk) -> Result<DecodingKey, PrivyError> {
    if jwk.kty != "EC" || jwk.crv != "P-256" {
        return Err(PrivyError::Invalid(format!(
            "unsupported key type {}/{} (expected EC/P-256)",
            jwk.kty, jwk.crv
        )));
    }
    DecodingKey::from_ec_components(&jwk.x, &jwk.y)
        .map_err(|e| PrivyError::Invalid(format!("bad EC key material: {e}")))
}

/// Verifies a Privy access token and returns its claims.
///
/// Checks, in order: token is well-formed; `kid` matches a published key
/// (refetching once on a miss); ES256 signature is valid; `exp` is in the
/// future; `iss` is `privy.io`; `aud` is one of the configured app IDs.
pub async fn verify_access_token(token: &str) -> Result<PrivyClaims, PrivyError> {
    let app_ids = app_ids();
    let primary = app_ids.first().ok_or(PrivyError::NotConfigured)?;

    let header = decode_header(token)
        .map_err(|e| PrivyError::Malformed(format!("unreadable header: {e}")))?;

    if header.alg != Algorithm::ES256 {
        return Err(PrivyError::Invalid(format!(
            "unexpected algorithm {:?} (expected ES256)",
            header.alg
        )));
    }

    let kid = header
        .kid
        .ok_or_else(|| PrivyError::Malformed("token header has no kid".into()))?;

    // First pass against the cache; on an unknown kid, refetch once — Privy
    // rotates keys, and a rotation should not cost a full TTL of failures.
    let mut jwks = get_jwks(primary, false).await?;
    if !jwks.keys.iter().any(|k| k.kid == kid) {
        tracing::info!("[Privy] unknown kid {kid}, refreshing key set");
        jwks = get_jwks(primary, true).await?;
    }

    let jwk = jwks
        .keys
        .iter()
        .find(|k| k.kid == kid)
        .ok_or_else(|| PrivyError::Invalid(format!("no published key for kid {kid}")))?;

    let key = decoding_key(jwk)?;

    let mut validation = Validation::new(Algorithm::ES256);
    validation.set_issuer(&[PRIVY_ISSUER]);
    validation.set_audience(&app_ids);
    validation.validate_exp = true;

    let data = decode::<PrivyClaims>(token, &key, &validation)
        .map_err(|e| PrivyError::Invalid(e.to_string()))?;

    Ok(data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    // These are pure on purpose: `cargo test` runs tests in parallel threads of
    // a SINGLE process, so any test that mutates PRIVY_APP_ID / PRIVY_JWKS_URL
    // races every other test that reads them. An earlier version of this file
    // did exactly that and failed intermittently.

    #[test]
    fn app_ids_parses_single_and_multi() {
        assert_eq!(parse_app_ids("app_one"), vec!["app_one"]);
        assert_eq!(
            parse_app_ids(" app_one , app_two ,"),
            vec!["app_one", "app_two"]
        );
    }

    #[test]
    fn app_ids_empty_means_unconfigured() {
        assert!(parse_app_ids("").is_empty());
        assert!(parse_app_ids("  ,  , ").is_empty());
    }

    #[test]
    fn jwks_url_derives_from_app_id() {
        assert_eq!(
            jwks_url_for("abc123", None),
            "https://auth.privy.io/api/v1/apps/abc123/jwks.json"
        );
        // A blank override must not win — an empty env var is "unset", not
        // "fetch the empty URL".
        assert_eq!(
            jwks_url_for("abc123", Some("  ")),
            "https://auth.privy.io/api/v1/apps/abc123/jwks.json"
        );
    }

    #[test]
    fn jwks_url_override_wins() {
        assert_eq!(
            jwks_url_for("ignored", Some("http://localhost:9/jwks.json")),
            "http://localhost:9/jwks.json"
        );
    }

    /// The real app's URL, so a refactor that breaks the path shape is caught
    /// here rather than by a 404 in production.
    #[test]
    fn jwks_url_matches_the_configured_app() {
        assert_eq!(
            jwks_url_for("cmt4t5mz200920ekywk04lspv", None),
            "https://auth.privy.io/api/v1/apps/cmt4t5mz200920ekywk04lspv/jwks.json"
        );
    }

    #[test]
    fn rejects_non_ec_key() {
        let jwk = Jwk {
            kty: "RSA".into(),
            crv: "P-256".into(),
            x: "x".into(),
            y: "y".into(),
            kid: "k".into(),
            alg: None,
        };
        assert!(decoding_key(&jwk).is_err());
    }

    /// The live endpoint returns two keys; this is the regression guard for the
    /// "just take keys[0]" shortcut.
    #[test]
    fn parses_a_two_key_privy_jwks() {
        let raw = r#"{"keys":[
            {"kty":"EC","x":"XhMlk9TEgcQVV9tXoxBt5JI_oxLdQ-dyc3i_SWgJTDI","y":"cLe_pRiMLnJJnVZ1Bfo0pIK_tnavWweOp7YVeO6h3EY","crv":"P-256","kid":"JA2V4sxfqwYeSq5P3PB_hjYZzRlWgkMIeqKkyl84puQ","use":"sig","alg":"ES256"},
            {"kty":"EC","x":"cUIiUNgHIuCnC5_Z-EkM6OSDtZTMh6lbNhe9hEWDdnI","y":"7W7B1c4-b-wmw6HNmI-GDjWhyIW_0bu5LvUyeRLfZ2Y","crv":"P-256","kid":"OjBjEkbF1sDKlgHqFPdVFt-gyyfXlzowmeofZq47To0","use":"sig","alg":"ES256"}
        ]}"#;
        let jwks: Jwks = serde_json::from_str(raw).expect("parses");
        assert_eq!(jwks.keys.len(), 2);
        assert!(jwks.keys.iter().all(|k| decoding_key(k).is_ok()));
        assert!(jwks
            .keys
            .iter()
            .any(|k| k.kid == "OjBjEkbF1sDKlgHqFPdVFt-gyyfXlzowmeofZq47To0"));
    }
}
