use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

const JWKS_CACHE_TTL: Duration = Duration::from_secs(3600);

const JWKS_MAX_KEYS: usize = 16;

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

#[derive(Debug, Clone, Deserialize)]
pub struct PrivyClaims {
    pub sub: String,
    pub iss: String,
    pub aud: String,
    pub exp: i64,
    #[serde(default)]
    pub iat: i64,
    #[serde(default)]
    pub sid: Option<String>,
}

#[derive(Debug)]
pub enum PrivyError {
    Jwks(String),
    Malformed(String),
    Invalid(String),
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

fn parse_app_ids(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn app_ids() -> Vec<String> {
    parse_app_ids(&std::env::var("PRIVY_APP_ID").unwrap_or_default())
}

pub fn is_configured() -> bool {
    !app_ids().is_empty()
}

fn jwks_url_for(app_id: &str, override_url: Option<&str>) -> String {
    match override_url.map(str::trim).filter(|s| !s.is_empty()) {
        Some(url) => url.to_string(),
        None => format!("https://auth.privy.io/api/v1/apps/{app_id}/jwks.json"),
    }
}

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
