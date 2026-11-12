use std::sync::{Mutex, RwLock};

const VPS_PROD_URL: &str = "https://xfchess.com";

static AUTH_TOKEN: RwLock<Option<String>> = RwLock::new(None);

/// The key this install's network node was created from. Its public half is the node id the
/// lobby relay, social graph and casual game log identify us by.
struct NodeKey {
    signing: ed25519_dalek::SigningKey,
    node_id: String,
    /// `(issued_at, token)`: reused until it is half-way to expiry so the pooled HTTP client
    /// (which is rebuilt whenever its default headers change) is not replaced on every call.
    token: Option<(i64, String)>,
}

static NODE_KEY: RwLock<Option<NodeKey>> = RwLock::new(None);

/// How long a node token is reused; the backend accepts one for an hour.
const NODE_TOKEN_REUSE_SECS: i64 = 30 * 60;

pub fn set_node_key(secret: Option<[u8; 32]>) {
    if let Ok(mut guard) = NODE_KEY.write() {
        *guard = secret.map(|bytes| {
            let signing = ed25519_dalek::SigningKey::from_bytes(&bytes);
            let node_id = bs58::encode(signing.verifying_key().as_bytes()).into_string();
            NodeKey {
                signing,
                node_id,
                token: None,
            }
        });
    }
}

/// `v1.<node_id>.<issued_at>.<signature>`, signed over what `backend::signing::node_auth`
/// verifies. Everything that acts for this node (lobby, friends, presence, game writes) sends it.
fn node_token(now: i64) -> Option<String> {
    use ed25519_dalek::Signer as _;
    let mut guard = NODE_KEY.write().ok()?;
    let key = guard.as_mut()?;
    if let Some((issued_at, token)) = &key.token {
        if now - issued_at < NODE_TOKEN_REUSE_SECS {
            return Some(token.clone());
        }
    }
    let signable = format!("xfchess-node-auth-v1:{}:{}", key.node_id, now);
    let signature = bs58::encode(key.signing.sign(signable.as_bytes()).to_bytes()).into_string();
    let token = format!("v1.{}.{}.{}", key.node_id, now, signature);
    key.token = Some((now, token.clone()));
    Some(token)
}

pub fn set_auth_token(token: Option<String>) {
    if let Ok(mut guard) = AUTH_TOKEN.write() {
        *guard = token;
    }
}

pub fn logout() {
    let token = AUTH_TOKEN.read().ok().and_then(|guard| guard.clone());
    set_auth_token(None);
    if let Some(token) = token {
        if let Ok(client) = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
        {
            let _ = client
                .post(format!("{}/api/auth/logout", vps_base()))
                .bearer_auth(token)
                .send();
        }
    }
}

pub fn vps_base() -> String {
    let override_url = std::env::var("SIGNING_SERVICE_URL")
        .or_else(|_| std::env::var("BACKEND_URL"))
        .ok();
    let allow_override = cfg!(debug_assertions)
        || std::env::var("XFCHESS_ALLOW_BACKEND_OVERRIDE")
            .ok()
            .as_deref()
            == Some("1");

    if allow_override {
        override_url.unwrap_or_else(|| VPS_PROD_URL.to_string())
    } else {
        VPS_PROD_URL.to_string()
    }
    .trim_end_matches('/')
    .to_string()
}

pub fn vps_ws_base() -> String {
    let base = vps_base();
    if let Some(rest) = base.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = base.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        base
    }
}

fn default_headers() -> reqwest::header::HeaderMap {
    let mut h = reqwest::header::HeaderMap::new();
    h.insert(
        "ngrok-skip-browser-warning",
        reqwest::header::HeaderValue::from_static("true"),
    );
    // Preferred per-user auth: a backend JWT obtained after wallet login.
    if let Ok(guard) = AUTH_TOKEN.read() {
        if let Some(token) = guard.as_ref() {
            if let Ok(value) = reqwest::header::HeaderValue::from_str(&format!("Bearer {token}")) {
                h.insert(reqwest::header::AUTHORIZATION, value);
            }
        }
    }
    // Proof of the network node id, for the routes that have no wallet.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or_default();
    if let Some(token) = node_token(now) {
        if let Ok(value) = reqwest::header::HeaderValue::from_str(&token) {
            h.insert("X-Node-Auth", value);
        }
    }
    // Legacy relay secret matching RELAY_SHARED_SECRET, sent with JWTs
    // while session-signing endpoints accept both authentication paths.
    if let Ok(secret) = std::env::var("RELAY_SHARED_SECRET") {
        if let Ok(value) = reqwest::header::HeaderValue::from_str(&secret) {
            h.insert("X-Relay-Secret", value);
        }
    }
    h
}

/// The headers every backend call carries (login token, relay secret), as name/value pairs
/// for transports that build their own requests.
pub fn auth_headers() -> Vec<(String, String)> {
    default_headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_owned(), value.to_owned()))
        })
        .collect()
}

pub fn client() -> Result<reqwest::blocking::Client, String> {
    pooled_client(&NORMAL_CLIENT, 120)
}

pub fn client_fast() -> Result<reqwest::blocking::Client, String> {
    pooled_client(&FAST_CLIENT, 15)
}

type CachedClient = Mutex<Option<(reqwest::header::HeaderMap, reqwest::blocking::Client)>>;
static NORMAL_CLIENT: CachedClient = Mutex::new(None);
static FAST_CLIENT: CachedClient = Mutex::new(None);

fn pooled_client(
    cache: &CachedClient,
    timeout_secs: u64,
) -> Result<reqwest::blocking::Client, String> {
    // JWT and relay-secret changes replace the pool before another caller can use it.
    // Cloning a reqwest client retains its connection pool.
    let mut cached = cache.lock().map_err(|e| format!("HTTP client lock: {e}"))?;
    let headers = default_headers();
    if let Some((previous, client)) = cached.as_ref() {
        if previous == &headers {
            return Ok(client.clone());
        }
    }
    let client = reqwest::blocking::Client::builder()
        .default_headers(headers.clone())
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;
    *cached = Some((headers, client.clone()));
    Ok(client)
}

// Keep wallet bridge port lookup feature-independent; menu polling also
// builds without the solana feature.

fn nominal_wallet_bridge_port() -> u16 {
    std::env::var("XFCHESS_WALLET_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(7454)
}

fn http_bridge_port_file() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "xfchess-wallet-http-{}.port",
        nominal_wallet_bridge_port()
    ))
}

pub fn wallet_bridge_port() -> u16 {
    if let Some(port) = std::env::var("XFCHESS_ACTUAL_WALLET_PORT")
        .ok()
        .and_then(|s| s.trim().parse().ok())
    {
        return port;
    }
    std::fs::read_to_string(http_bridge_port_file())
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or_else(nominal_wallet_bridge_port)
}

#[derive(Debug, Clone)]
pub struct SolUsdRateResponse {
    pub sol_per_usd: f64,
    pub usd_per_sol: f64,
    pub fetched_at: i64,
}

pub fn fetch_sol_usd_rate() -> Result<SolUsdRateResponse, String> {
    let resp = client()?
        .get(format!("{}/api/rates/all", vps_base()))
        .send()
        .map_err(|e| format!("vps fetch_sol_usd_rate: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(format!("vps fetch_sol_usd_rate: HTTP {status} - {body}"));
    }

    let json: serde_json::Value = resp
        .json()
        .map_err(|e| format!("vps fetch_sol_usd_rate parse: {e}"))?;

    let usd_per_sol = json["rates"]["usd"]
        .as_f64()
        .ok_or("vps fetch_sol_usd_rate: missing rates.usd")?;
    let sol_per_usd = json["sol_per_fiat"]["usd"]
        .as_f64()
        .unwrap_or(if usd_per_sol > 0.0 {
            1.0 / usd_per_sol
        } else {
            0.0
        });
    let fetched_at = json["fetched_at"].as_i64().unwrap_or(0);

    Ok(SolUsdRateResponse {
        sol_per_usd,
        usd_per_sol,
        fetched_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    /// The node key is process-global, so tests that set it take turns.
    static NODE_KEY_TESTS: Mutex<()> = Mutex::new(());

    /// Issued at t=1000 for the key `[7; 32]`. Ed25519 is deterministic, so this is exact.
    const PINNED_TOKEN: &str = "v1.GmaDrppBC7P5ARKV8g3djiwP89vz1jLK23V2GBjuAEGB.1000.4gHhj4H43dJeVWhjFZgTGo1p24TDwtiD8Box3AEzXRsmTQ3BUi3WV4pw8tYGrgLSkRywFhRCpZmJE8dthWQoRu7M";

    #[test]
    fn node_token_has_the_shape_the_backend_verifies() {
        let _turn = NODE_KEY_TESTS.lock();
        use ed25519_dalek::Verifier as _;
        set_node_key(Some([7u8; 32]));
        let token = node_token(1_000).unwrap_or_default();
        let parts: Vec<&str> = token.split('.').collect();
        assert_eq!(parts.len(), 4, "{token}");
        // Also verified by the backend in `signing::node_auth` tests: the two must not drift.
        assert_eq!(token, PINNED_TOKEN);
        assert_eq!(parts[0], "v1");
        assert_eq!(parts[2], "1000");
        // The same bytes the backend rebuilds: prefix, node id, issue time.
        let key = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
        assert_eq!(
            parts[1],
            bs58::encode(key.verifying_key().as_bytes()).into_string()
        );
        let signature = bs58::decode(parts[3]).into_vec().unwrap_or_default();
        let signature = ed25519_dalek::Signature::from_slice(&signature);
        assert!(signature.is_ok());
        let signable = format!("xfchess-node-auth-v1:{}:1000", parts[1]);
        assert!(signature
            .map(|signature| key
                .verifying_key()
                .verify(signable.as_bytes(), &signature)
                .is_ok())
            .unwrap_or(false));
        set_node_key(None);
    }

    #[test]
    fn node_token_is_reused_until_half_expiry() {
        let _turn = NODE_KEY_TESTS.lock();
        set_node_key(Some([9u8; 32]));
        let first = node_token(1_000);
        assert_eq!(node_token(1_000 + NODE_TOKEN_REUSE_SECS - 1), first);
        assert_ne!(node_token(1_000 + NODE_TOKEN_REUSE_SECS), first);
        set_node_key(None);
        assert_eq!(node_token(1_000), None);
    }

    #[test]
    fn pool_reuses_connections_and_replaces_auth_on_login_and_logout(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let url = format!("http://{}", listener.local_addr()?);
        let server = std::thread::spawn(
            move || -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
                let mut requests = Vec::new();
                // The first two requests reuse a socket; changing credentials creates
                // a new pool for the subsequent two requests.
                for count in [2, 1, 1] {
                    let (mut socket, _) = listener.accept()?;
                    socket.set_read_timeout(Some(std::time::Duration::from_secs(10)))?;
                    for _ in 0..count {
                        let mut header = Vec::new();
                        while !header.ends_with(b"\r\n\r\n") {
                            let mut byte = [0];
                            socket.read_exact(&mut byte)?;
                            header.push(byte[0]);
                        }
                        requests.push(String::from_utf8(header)?.to_ascii_lowercase());
                        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")?;
                    }
                }
                Ok(requests)
            },
        );
        let cache = Mutex::new(None);
        set_auth_token(Some("first".into()));
        for _ in 0..2 {
            pooled_client(&cache, 5)?.get(&url).send()?.text()?;
        }
        set_auth_token(Some("second".into()));
        pooled_client(&cache, 5)?.get(&url).send()?.text()?;
        set_auth_token(None);
        pooled_client(&cache, 5)?.get(&url).send()?.text()?;
        let requests = server
            .join()
            .map_err(|_| "HTTP fixture thread panicked")?
            .map_err(|error| error.to_string())?;
        assert!(requests[0].contains("authorization: bearer first\r\n"));
        assert!(requests[1].contains("authorization: bearer first\r\n"));
        assert!(requests[2].contains("authorization: bearer second\r\n"));
        assert!(!requests[3].contains("authorization:"));

        Ok(())
    }
}
