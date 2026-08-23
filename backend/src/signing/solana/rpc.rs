//! RPC client helpers for Solana — resilient, provider-agnostic.
//!
//! Primary provider is whatever `SOLANA_RPC_URL` points at (we run **Triton One**
//! `*.rpcpool.com` in prod; the x-token is embedded in the URL path and is a secret,
//! so it lives only in the untracked `.env` and is **redacted** before any logging).
//! `SOLANA_RPC_FALLBACK_URL` (default: public devnet) is used when the primary is
//! failing, guarded by a lightweight circuit breaker so we don't hammer a dead endpoint.
//!
//! Every client created here carries request/connect **timeouts** — no RPC call can hang
//! the backend indefinitely.

use solana_client::client_error::Result as ClientResult;
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::hash::Hash;
use std::sync::atomic::{AtomicI64, AtomicU32, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Default per-request timeout for all RPC clients.
const RPC_TIMEOUT: Duration = Duration::from_secs(30);
/// Consecutive primary failures before the breaker opens.
const BREAKER_THRESHOLD: u32 = 3;
/// How long the breaker stays open (skip primary, use fallback) once tripped.
const BREAKER_COOLDOWN_SECS: i64 = 30;

/// Creates an RPC client (confirmed commitment) with a bounded request timeout.
pub fn make_rpc(url: &str) -> RpcClient {
    RpcClient::new_with_timeout_and_commitment(
        url.to_string(),
        RPC_TIMEOUT,
        CommitmentConfig::confirmed(),
    )
}

/// Primary RPC URL (Triton in prod). Falls back to public devnet only if unset.
pub fn rpc_url_or_devnet() -> String {
    std::env::var("SOLANA_RPC_URL").unwrap_or_else(|_| "https://api.devnet.solana.com".to_string())
}

/// Fallback RPC URL used when the primary is unhealthy.
pub fn fallback_rpc_url() -> String {
    std::env::var("SOLANA_RPC_FALLBACK_URL")
        .unwrap_or_else(|_| "https://api.devnet.solana.com".to_string())
}

/// Strip secrets (rpcpool x-token in the path, or an `api-key`/`x-token` query) from an
/// RPC URL so it is safe to log. e.g. `https://x.rpcpool.com/<token>` -> `https://x.rpcpool.com/***`.
pub fn redact_url(url: &str) -> String {
    // Drop any query string entirely (may carry api-key=...).
    let base = url.split('?').next().unwrap_or(url);
    match base.split_once("://") {
        Some((scheme, rest)) => {
            let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
            if path.is_empty() {
                format!("{scheme}://{host}")
            } else {
                format!("{scheme}://{host}/***")
            }
        }
        None => "***".to_string(),
    }
}

// ── Circuit breaker for the primary endpoint ────────────────────────────────
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
    let n = PRIMARY_FAILURES.fetch_add(1, Ordering::Relaxed) + 1;
    if n >= BREAKER_THRESHOLD {
        PRIMARY_OPEN_UNTIL.store(now_secs() + BREAKER_COOLDOWN_SECS, Ordering::Relaxed);
        tracing::warn!(
            "[rpc] primary circuit breaker OPEN for {}s after {} consecutive failures",
            BREAKER_COOLDOWN_SECS,
            n
        );
    }
}

/// Fetches a blockhash for a transaction that a **browser wallet extension**
/// (Phantom/Solflare) will be asked to sign, at `finalized` commitment rather
/// than the `confirmed` default `make_rpc` sets.
///
/// This is not about durability, it is about the wallet being able to tell
/// which cluster the transaction belongs to. An extension has no way of
/// knowing that from the transaction bytes except by looking the blockhash up
/// on the cluster it is currently set to — and `isBlockhashValid` defaults to
/// `finalized` commitment. A blockhash fetched at `confirmed` is younger than
/// the ~32 slot (~13s) finalization lag, so that lookup returns **false** for
/// a perfectly good devnet blockhash. Solflare reads that as "not a devnet
/// transaction" and refuses to sign with "Network mismatch: your current
/// network is set to devnet, but this transaction is for mainnet". Measured
/// against our own devnet endpoint vs. a second devnet node:
///
/// ```text
/// getLatestBlockhash{confirmed} -> isBlockhashValid{processed} = true
///                               -> isBlockhashValid{finalized} = false  <- what the wallet sees
/// getLatestBlockhash{finalized} -> isBlockhashValid{finalized} = true
/// ```
///
/// The cost is ~13s of the ~60s validity window (a finalized blockhash is
/// already ~32 slots old). The signing paths already retry once on a stale
/// blockhash, and a slightly shorter window beats a transaction the wallet
/// refuses to sign at all.
///
/// Only for transactions leaving the backend to be signed by a user wallet.
/// Backend-signed transactions (settlement, treasury, cranks) broadcast
/// immediately and should keep the faster `confirmed` default.
pub fn wallet_signable_blockhash(rpc: &RpcClient) -> ClientResult<Hash> {
    rpc.get_latest_blockhash_with_commitment(CommitmentConfig::finalized())
        .map(|(hash, _last_valid_block_height)| hash)
}

/// Run a **read-only** RPC operation against the primary, transparently failing over to
/// the fallback endpoint. A circuit breaker skips the primary during a cooldown after
/// repeated failures. Use for idempotent reads (account/PDA fetches, `get_*`), NOT for
/// `send_transaction` (which is not safe to blindly retry on another endpoint).
///
/// ```ignore
/// let acct = read_with_failover(|rpc| rpc.get_account_data(&pda))?;
/// ```
pub fn read_with_failover<T, F>(op: F) -> ClientResult<T>
where
    F: Fn(&RpcClient) -> ClientResult<T>,
{
    // If the breaker is open, go straight to the fallback.
    if breaker_is_open() {
        let fb = fallback_rpc_url();
        tracing::debug!("[rpc] breaker open — using fallback {}", redact_url(&fb));
        return op(&make_rpc(&fb));
    }

    let primary = rpc_url_or_devnet();
    match op(&make_rpc(&primary)) {
        Ok(v) => {
            record_primary_success();
            Ok(v)
        }
        Err(primary_err) => {
            record_primary_failure();
            let fb = fallback_rpc_url();
            // Only bother failing over if the fallback is a *different* endpoint.
            if redact_url(&fb) == redact_url(&primary) {
                return Err(primary_err);
            }
            tracing::warn!(
                "[rpc] primary {} failed ({}), failing over to {}",
                redact_url(&primary),
                primary_err,
                redact_url(&fb)
            );
            op(&make_rpc(&fb))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_strips_rpcpool_token_path() {
        assert_eq!(
            redact_url("https://xfsoluti-solanad-d155.devnet.rpcpool.com/df802762-token"),
            "https://xfsoluti-solanad-d155.devnet.rpcpool.com/***"
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
