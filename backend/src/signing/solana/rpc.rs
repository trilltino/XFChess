use parking_lot::Mutex;
use solana_client::client_error::Result as ClientResult;
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::hash::Hash;
use std::sync::atomic::{AtomicI64, AtomicU32, Ordering};
use std::sync::LazyLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const RPC_TIMEOUT: Duration = Duration::from_secs(30);
const RPC_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const RPC_POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(5);
const RPC_TCP_KEEPALIVE: Option<Duration> = Some(Duration::from_secs(15));
const BREAKER_THRESHOLD: u32 = 3;
const BREAKER_COOLDOWN_SECS: i64 = 30;

pub fn make_rpc(url: &str) -> ClientResult<RpcClient> {
    // The blocking RpcClient drives requests on a private runtime that is shut
    // down when the client is dropped. Connections it opened die with it, so it
    // must not return them to a pool shared with later clients: the next
    // request would pick one up and stall until the full request timeout.
    Ok(RpcClient::new_sender(
        measured_sender(url, false)?,
        solana_client::rpc_client::RpcClientConfig::with_commitment(CommitmentConfig::confirmed()),
    ))
}

pub fn make_async_rpc(
    url: &str,
) -> ClientResult<solana_client::nonblocking::rpc_client::RpcClient> {
    Ok(
        solana_client::nonblocking::rpc_client::RpcClient::new_sender(
            measured_sender(url, true)?,
            solana_client::rpc_client::RpcClientConfig::with_commitment(
                CommitmentConfig::confirmed(),
            ),
        ),
    )
}

/// `shared` clients reuse one connection pool per URL and are only safe for
/// async callers on the long-lived server runtime; the rest get a private pool.
fn measured_sender(
    url: &str,
    shared: bool,
) -> ClientResult<solana_rpc_client::http_sender::HttpSender> {
    use solana_client::client_error::reqwest;
    static CLIENTS: LazyLock<
        Mutex<std::collections::HashMap<String, reqwest_middleware::ClientWithMiddleware>>,
    > = LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));
    crate::telemetry::rpc::register(url);
    if !shared {
        let client = reqwest::Client::builder()
            .timeout(RPC_TIMEOUT)
            .connect_timeout(RPC_CONNECT_TIMEOUT)
            .default_headers(solana_rpc_client::http_sender::HttpSender::default_headers())
            .build()?;
        let client = reqwest_middleware::ClientBuilder::new(client)
            .with(crate::telemetry::rpc::RpcMetrics)
            .build();
        return Ok(
            solana_rpc_client::http_sender::HttpSender::new_with_client_with_middleware(
                url, client,
            ),
        );
    }
    if let Some(client) = CLIENTS.lock().get(url).cloned() {
        return Ok(
            solana_rpc_client::http_sender::HttpSender::new_with_client_with_middleware(
                url, client,
            ),
        );
    }
    // A pooled connection that a NAT or the provider dropped silently stalls
    // the next request until its full timeout, so idle connections are retired
    // quickly and kept alive at the TCP level.
    let client = reqwest::Client::builder()
        .timeout(RPC_TIMEOUT)
        .connect_timeout(RPC_CONNECT_TIMEOUT)
        .pool_idle_timeout(RPC_POOL_IDLE_TIMEOUT)
        .tcp_keepalive(RPC_TCP_KEEPALIVE)
        .default_headers(solana_rpc_client::http_sender::HttpSender::default_headers())
        .build()?;
    let client = reqwest_middleware::ClientBuilder::new(client)
        .with(crate::telemetry::rpc::RpcMetrics)
        .build();
    let mut clients = CLIENTS.lock();
    // Only configured/current endpoints need a persistent connection pool.
    if clients.len() >= 256 {
        clients.clear();
    }
    let client = clients.entry(url.to_owned()).or_insert(client).clone();
    Ok(solana_rpc_client::http_sender::HttpSender::new_with_client_with_middleware(url, client))
}

pub fn rpc_url_or_devnet() -> String {
    std::env::var("SOLANA_RPC_URL").unwrap_or_else(|_| "https://api.devnet.solana.com".to_string())
}

pub fn fallback_rpc_url() -> String {
    std::env::var("SOLANA_RPC_FALLBACK_URL")
        .unwrap_or_else(|_| "https://api.devnet.solana.com".to_string())
}

pub fn redact_url(url: &str) -> String {
    let Ok(url) = reqwest::Url::parse(url) else {
        return "***".into();
    };
    let Some(host) = url.host_str() else {
        return "***".into();
    };
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    let port = url
        .port()
        .map(|port| format!(":{port}"))
        .unwrap_or_default();
    let path = if url.path().is_empty() || url.path() == "/" {
        ""
    } else {
        "/***"
    };
    format!("{}://{host}{port}{path}", url.scheme())
}

/// Redacts every `http(s)://` URL embedded in free text, such as the
/// `error sending request for url (...)` that reqwest puts in its error
/// Display. RPC provider tokens live in the URL path, so error strings that
/// reach HTTP responses or logs must pass through this.
pub fn redact_urls_in(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = ["https://", "http://"]
        .iter()
        .filter_map(|scheme| rest.find(scheme))
        .min()
    {
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        let end = tail
            .find(|c: char| c.is_whitespace() || matches!(c, ')' | '"' | '\'' | '<' | '>' | ',' | '\u{1b}'))
            .unwrap_or(tail.len());
        out.push_str(&redact_url(&tail[..end]));
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

static PRIMARY_FAILURES: AtomicU32 = AtomicU32::new(0);
static PRIMARY_OPEN_UNTIL: AtomicI64 = AtomicI64::new(0);

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn breaker_is_open() -> bool {
    now_secs() < PRIMARY_OPEN_UNTIL.load(Ordering::Relaxed)
}

fn record_primary_success() {
    PRIMARY_FAILURES.store(0, Ordering::Relaxed);
}

fn record_primary_failure() {
    let n = PRIMARY_FAILURES
        .fetch_add(1, Ordering::Relaxed)
        .saturating_add(1);
    if n >= BREAKER_THRESHOLD {
        PRIMARY_OPEN_UNTIL.store(
            now_secs().saturating_add(BREAKER_COOLDOWN_SECS),
            Ordering::Relaxed,
        );
        tracing::warn!(
            "[rpc] primary circuit breaker OPEN for {}s after {} consecutive failures",
            BREAKER_COOLDOWN_SECS,
            n
        );
    }
}

pub fn wallet_signable_blockhash(rpc: &RpcClient) -> ClientResult<Hash> {
    rpc.get_latest_blockhash_with_commitment(CommitmentConfig::finalized())
        .map(|(hash, _last_valid_block_height)| hash)
}

fn same_endpoint(first: &str, second: &str) -> bool {
    match (reqwest::Url::parse(first), reqwest::Url::parse(second)) {
        (Ok(first), Ok(second)) => first == second,
        _ => first == second,
    }
}

pub fn read_with_failover<T, F>(op: F) -> ClientResult<T>
where
    F: Fn(&RpcClient) -> ClientResult<T>,
{
    // If the breaker is open, go straight to the fallback.
    if breaker_is_open() {
        let fb = fallback_rpc_url();
        tracing::debug!("[rpc] breaker open — using fallback {}", redact_url(&fb));
        return op(&make_rpc(&fb)?);
    }

    let primary = rpc_url_or_devnet();
    match op(&make_rpc(&primary)?) {
        Ok(v) => {
            record_primary_success();
            Ok(v)
        }
        Err(primary_err) => {
            record_primary_failure();
            let fb = fallback_rpc_url();
            // Only bother failing over if the fallback is a *different* endpoint.
            if same_endpoint(&fb, &primary) {
                return Err(primary_err);
            }
            tracing::warn!(
                "[rpc] primary {} failed; failing over to {}",
                redact_url(&primary),
                redact_url(&fb)
            );
            op(&make_rpc(&fb)?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redaction_removes_user_credentials_and_fragment() {
        assert_eq!(
            redact_url("https://user:password@rpc.example/secret?key=hidden#private"),
            "https://rpc.example/***"
        );
    }

    #[test]
    fn endpoint_identity_keeps_credentials_and_paths() {
        assert!(!same_endpoint(
            "https://rpc.example/a",
            "https://rpc.example/b"
        ));
        assert!(!same_endpoint(
            "https://rpc.example/?key=a",
            "https://rpc.example/?key=b"
        ));
        assert!(same_endpoint("https://rpc.example", "https://rpc.example/"));
    }

    #[test]
    fn redact_strips_rpcpool_token_path() {
        assert_eq!(
            redact_url("https://example-node.devnet.rpcpool.com/abcd1234-token"),
            "https://example-node.devnet.rpcpool.com/***"
        );
    }

    #[test]
    fn redact_strips_query_apikey() {
        assert_eq!(
            redact_url("https://mainnet.helius-rpc.com/?api-key=secret"),
            "https://mainnet.helius-rpc.com"
        );
    }

    #[test]
    fn redact_urls_in_scrubs_reqwest_error_text() {
        let text = "could not establish rent-payer balance for B34M: error sending request for url (https://example-node.devnet.rpcpool.com/abcd1234-token)";
        let scrubbed = redact_urls_in(text);
        assert!(!scrubbed.contains("abcd1234"));
        assert!(scrubbed.contains("(https://example-node.devnet.rpcpool.com/***)"));
        assert_eq!(redact_urls_in("no urls here"), "no urls here");
    }

    #[test]
    fn redact_leaves_plain_host() {
        assert_eq!(
            redact_url("https://api.devnet.solana.com"),
            "https://api.devnet.solana.com"
        );
    }

    #[test]
    fn breaker_opens_after_threshold() {
        PRIMARY_FAILURES.store(0, Ordering::Relaxed);
        PRIMARY_OPEN_UNTIL.store(0, Ordering::Relaxed);
        for _ in 0..BREAKER_THRESHOLD {
            record_primary_failure();
        }
        assert!(
            breaker_is_open(),
            "breaker should be open after threshold failures"
        );
        record_primary_success();
        // success resets the counter (breaker window still elapses on its own)
        assert_eq!(PRIMARY_FAILURES.load(Ordering::Relaxed), 0);
    }
}
