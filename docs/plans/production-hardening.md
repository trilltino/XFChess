# Production Hardening Plan

Current state: functionally complete, serves real users. These are the gaps
between "it works" and "it works reliably under adversarial conditions."

Gaps are ordered by impact severity — fix the top ones before launch.

---

## Gap 1 — Tournament state is lost on VPS restart

**Severity: Critical**

`backend/src/tournament/store.rs` is a `Arc<RwLock<HashMap>>`. Nothing writes
it to disk. A deploy, a crash, or a kernel update in the middle of a live
tournament destroys all bracket state. Players have staked SOL and their
tournament simply ceases to exist.

**Fix:**
Persist the tournament store to SQLite on every state transition. The store
already serializes to JSON (`#[derive(Serialize, Deserialize)]`). Add a
`save_to_db(pool)` call at the end of every mutation and a `load_from_db(pool)`
call at startup. The migration is one new table:

```sql
CREATE TABLE IF NOT EXISTS tournament_snapshots (
    tournament_id TEXT PRIMARY KEY,
    state         TEXT NOT NULL,        -- JSON
    updated_at    INTEGER NOT NULL
);
```

This is a one-day implementation. Without it, a production tournament cannot
be run safely.

---

## Gap 2 — No database backup

**Severity: Critical**

The SQLite file is on one Hetzner disk. Disk failure = all ELO history,
game records, player profiles, and tournament results gone permanently.
WAL mode protects against corruption from crashes, not hardware failure.

**Fix:**
Add a nightly cron on the VPS that:
1. Runs `sqlite3 xfchess.db ".backup /tmp/xfchess-$(date +%Y%m%d).db"`
2. Uploads to Hetzner Object Storage (S3-compatible, ~€5/month) or Backblaze B2
3. Keeps 30 days of retention

Script lives in `scripts/backup_db.sh`. Add a cron entry via `crontab -e`:
```
0 3 * * * /opt/xfchess/scripts/backup_db.sh >> /var/log/xfchess-backup.log 2>&1
```

Also validate the backup weekly: restore to a temp path and run a row count
check. A backup that has never been tested is not a backup.

---

## Gap 3 — No alerting (Prometheus metrics with no alerts)

**Severity: High**

Prometheus metrics are collected. Grafana dashboards exist. But nothing pages
anyone when the backend crashes, the fee payer runs low, or Solana RPC errors
spike. You find out from a player complaint, not before it.

**Fix — three components:**

**Alertmanager rules** (`monitoring/alerts.yml`):

```yaml
groups:
  - name: xfchess
    rules:
      - alert: BackendDown
        expr: up{job="xfchess-backend"} == 0
        for: 1m
        annotations:
          summary: "Backend is not responding"

      - alert: FeePayerLow
        expr: feepayer_balance_lamports < 50000000   # 0.05 SOL
        for: 5m
        annotations:
          summary: "Fee payer balance critical — game creation will fail"

      - alert: SolanaRpcErrorRate
        expr: rate(solana_rpc_errors_total[5m]) > 0.1
        for: 2m
        annotations:
          summary: "Solana RPC error rate above 10%"

      - alert: HighHttpErrorRate
        expr: rate(http_requests_total{status=~"5.."}[5m]) > 0.05
        for: 3m
        annotations:
          summary: "Backend 5xx rate above 5%"
```

**Alertmanager → Slack webhook** (cheapest oncall that actually wakes you up):
```yaml
receivers:
  - name: slack
    slack_configs:
      - api_url: $SLACK_WEBHOOK_URL
        channel: '#xfchess-alerts'
```

**PagerDuty** (if you need phone calls for critical alerts):
Add `pagerduty_configs` to the `BackendDown` and `FeePayerLow` alerts only.
Not every alert needs to wake someone at 3am.

---

## Gap 4 — No Solana RPC fallback

**Severity: High**

`backend/src/signing/solana/rpc.rs` reads `SOLANA_RPC_URL` or falls back to
`api.devnet.solana.com`. There is one URL, no retries with backoff, no
failover to a second provider. If Helius/Triton/QuickNode has an outage, every
game creation and finalization call fails until you manually update the env var
and restart.

**Fix:**
Add a fallback URL env var and try it on `Err`:

```rust
pub fn make_rpc_with_fallback() -> RpcClient {
    let primary = std::env::var("SOLANA_RPC_URL")
        .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
    RpcClient::new_with_commitment(primary, CommitmentConfig::confirmed())
}

// In the route handler, on RPC error:
// if primary fails, retry once against SOLANA_RPC_FALLBACK_URL
```

For production, use two different providers (e.g., Helius primary, Triton
fallback). They have different infrastructure so a single-provider outage
doesn't take both down.

---

## Gap 5 — No graceful shutdown / in-flight move drain

**Severity: High**

When you `systemctl restart xfchess-backend` mid-game, Axum's default SIGTERM
handling drops all active connections immediately. Any move in flight between
the client and backend is lost. Players see a disconnect mid-game.

**Fix:**
Add a SIGTERM handler that:
1. Stops accepting new connections
2. Waits up to 30 seconds for in-flight requests to complete
3. Only then exits

Axum supports this via `axum_server::Handle`:

```rust
let handle = axum_server::Handle::new();
let shutdown_handle = handle.clone();

tokio::spawn(async move {
    tokio::signal::unix::signal(SignalKind::terminate())
        .unwrap()
        .recv()
        .await;
    shutdown_handle.graceful_shutdown(Some(Duration::from_secs(30)));
});
```

Add this to `backend/src/main.rs` before `axum_server::bind(...).serve(...)`.

---

## Gap 6 — No load testing / unknown SQLite ceiling

**Severity: Medium**

SQLite with WAL mode and `max_connections = 16` works well under light load.
The write path serializes — one writer at a time. Under 100 concurrent games
(200 players) submitting moves simultaneously, this becomes a bottleneck. The
exact ceiling is unknown because no load test has been run.

**Fix — two steps:**

**Step 1: Benchmark before assuming you need to migrate.**
Run `wrk` or `k6` against the move submission endpoint with 50/100/200
concurrent virtual users. Find the point where p99 latency exceeds 500ms or
error rate exceeds 1%. That number is your current ceiling.

```bash
k6 run scripts/load_test_moves.js
```

Write a simple `scripts/load_test_moves.js` that hammers `POST /api/game/{id}/move`.

**Step 2: If ceiling < 500 concurrent games, migrate to PostgreSQL.**
SQLite is appropriate for dev and small-scale prod. The migration path:
- Replace `sqlx::SqlitePool` with `sqlx::PgPool` (same query syntax, SQLx
  handles both)
- Migrate schema: `sqitch` or `dbmate` for repeatable migrations
- Run PostgreSQL on a separate Hetzner node (€5/month CX11) or use Supabase

Do not migrate speculatively. Run the load test first.

---

## Gap 7 — No global rate limiting

**Severity: Medium**

IP-based anti-cheat checks exist in `signing/blinks/anti_cheat.rs` but they
are per-route. There is no global rate limiter preventing a single IP from
exhausting the connection pool by hammering arbitrary endpoints.

**Fix:**
Add `tower_governor` middleware in `infrastructure/router.rs`:

```rust
use tower_governor::{GovernorConfigBuilder, GovernorLayer};

let governor_conf = GovernorConfigBuilder::default()
    .per_second(20)       // 20 requests per second per IP
    .burst_size(50)       // allow bursts up to 50
    .finish()
    .unwrap();

let app = Router::new()
    // ... routes ...
    .layer(GovernorLayer { config: Arc::new(governor_conf) });
```

Alternatively, configure rate limiting upstream in nginx (simpler, no code
change):
```nginx
limit_req_zone $binary_remote_addr zone=xfchess:10m rate=20r/s;
limit_req zone=xfchess burst=50 nodelay;
```

---

## Gap 8 — Fee payer balance not automatically topped up

**Severity: Medium**

The fee payer wallet pays Solana transaction fees for game creation, joins, and
record_move. `feepayer_balance_lamports` is in the metrics but nothing tops
it up automatically. When it empties, every transaction fails silently from
the user's perspective (they just get a generic error).

**Fix:**
- Set the `FeePayerLow` alert from Gap 3
- Document the top-up procedure: `solana transfer <fee_payer_pubkey> 1 --allow-unfunded-recipient`
- Optionally: add a background task that checks balance every 6 hours and
  triggers a Slack notification when below 0.1 SOL. The task can live in
  `backend/src/tasks/fee_claimer.rs` which already monitors fee activity.

---

## Gap 9 — No TLS termination documented

**Severity: Medium**

The backend listens on HTTP. For production it must be behind a TLS-terminating
reverse proxy. If this isn't set up, wallet signatures and session keys travel
in plaintext.

**Fix:**
If not already done, add nginx + Certbot on the VPS:

```nginx
server {
    listen 443 ssl;
    server_name api.xfchess.gg;

    ssl_certificate     /etc/letsencrypt/live/api.xfchess.gg/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/api.xfchess.gg/privkey.pem;

    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }
}
```

Certbot auto-renews via a systemd timer. Document the renewal check:
`certbot renew --dry-run`.

---

## Gap 10 — Solana RPC subscription on devnet only

**Severity: Low (pre-mainnet)**

`backend/src/tasks/mod.rs` hardcodes `Cluster::Devnet` and
`wss://devnet-eu.magicblock.app` for the WebSocket subscriber. Before mainnet
launch, these must be environment-variable driven, not hardcoded.

**Fix:**
```rust
let cluster = match std::env::var("SOLANA_NETWORK").as_deref() {
    Ok("mainnet") => Cluster::Mainnet,
    _ => Cluster::Devnet,
};
let mb_url = std::env::var("MAGICBLOCK_WS_URL")
    .unwrap_or_else(|_| "wss://devnet-eu.magicblock.app".to_string());
```

---

## Gap 11 — No player data export / deletion endpoint (GDPR)

**Severity: Low (legal exposure if EU players)**

A `vault_pool` exists for GDPR-compliant encrypted storage. But there is no
`DELETE /api/player/me` or `GET /api/player/me/export` endpoint. If an EU
player requests their data be deleted, there is no automated path to fulfill it.

**Fix:**
Two endpoints in `signing/routes/`:
- `GET /api/player/me/export` — returns JSON of all player data (profile, ELO
  history, game records, session keys)
- `DELETE /api/player/me` — soft-deletes profile, purges vault entry, anonymises
  game records (replace pubkey with `[deleted]`)

---

## Implementation Priority

| Priority | Gap | Effort |
|---|---|---|
| **P0 — do before launch** | Gap 1: Persist tournament state | 1 day |
| **P0 — do before launch** | Gap 2: Automated DB backup | 2 hours |
| **P0 — do before launch** | Gap 3: Alerting + Slack webhook | 4 hours |
| **P1 — do first week** | Gap 4: Solana RPC fallback | 2 hours |
| **P1 — do first week** | Gap 5: Graceful shutdown drain | 2 hours |
| **P1 — do first week** | Gap 7: Global rate limiting | 2 hours |
| **P2 — do first month** | Gap 6: Load test + ceiling | 1 day |
| **P2 — do first month** | Gap 8: Fee payer alert + runbook | 2 hours |
| **P2 — do first month** | Gap 9: TLS documentation | 1 hour |
| **P3 — pre-mainnet** | Gap 10: Devnet hardcodes | 1 hour |
| **P3 — EU players only** | Gap 11: GDPR endpoints | 2 days |

P0 items (Gaps 1, 2, 3) together are a hard requirement before real money is
at stake in tournaments. Everything else is hardening that can ship alongside
features.

---

## Additional Gaps (Second Pass)

---

## Gap 12 — Hardcoded Helius API key in source

**Severity: Critical**

`backend/src/signing/routes/wallet.rs:27-28` and `rates.rs:33-34`:

```rust
let key = std::env::var("HELIUS_API_KEY")
    .unwrap_or_else(|_| "5bb5fed2-8d33-458b-b7d2-3d18fdbb3da5".to_string());
```

This key is in the git history and visible to anyone with repo access. Once
Helius detects it is in public code (or a third party abuses it), it gets
rate-limited or revoked. Wallet balance fetches and rate lookups then silently
degrade.

**Fix:**
Remove the fallback entirely. Fail fast on startup if `HELIUS_API_KEY` is
missing:
```rust
let key = std::env::var("HELIUS_API_KEY")
    .expect("HELIUS_API_KEY must be set");
```
Rotate the exposed key immediately regardless of anything else.

---

## Gap 13 — Permissive CORS allows any origin

**Severity: High**

`backend/src/infrastructure/router.rs:100`:
```rust
tower_http::cors::CorsLayer::permissive()
```

`permissive()` sets `Access-Control-Allow-Origin: *`. Any website can make
credentialed requests to the API from a user's browser. Combined with auth
endpoints, this is a CSRF vector.

**Fix:**
```rust
CorsLayer::new()
    .allow_origin([
        "https://xfchess.gg".parse::<HeaderValue>().unwrap(),
    ])
    .allow_methods([Method::GET, Method::POST])
    .allow_headers([AUTHORIZATION, CONTENT_TYPE])
    .allow_credentials(true)
```

---

## Gap 14 — No request body size limit

**Severity: High**

Axum has no default max body size. An attacker can POST a 1 GB JSON payload
and exhaust memory on the VPS. No middleware in `router.rs` sets a limit.

**Fix:**
```rust
use axum::extract::DefaultBodyLimit;

let app = Router::new()
    .layer(DefaultBodyLimit::max(1024 * 1024)); // 1 MB
```

---

## Gap 15 — Critical keypairs silently replaced with random keys

**Severity: Critical**

`backend/src/signing/mod.rs:179-197` — when `vps_authority_key`,
`kyc_authority_key`, or `link_authority_key` env vars are absent, the
backend generates a fresh random `Keypair::new()` and logs a warning then
continues:

```rust
warn!("[VPS] No vps_authority_key provided, using random fallback");
Keypair::new()
```

The server starts and appears healthy. But every Solana transaction it signs
uses a throwaway key not authorized on-chain. Game creation, KYC, and session
delegation all fail silently at the RPC level.

**Fix:**
```rust
let vps_authority = std::env::var("VPS_AUTHORITY_KEY")
    .expect("VPS_AUTHORITY_KEY is required — backend cannot sign transactions without it");
```
Apply the same to `kyc_authority_key` and `link_authority_key`.

---

## Gap 16 — SIWS nonce map grows without bound

**Severity: Medium**

`backend/src/signing/mod.rs:131`:
```rust
pub siws_nonces: Arc<Mutex<HashMap<String, (String, u64)>>>,
```

Nonces store an expiry timestamp but no cleanup task ever removes expired
entries. Every abandoned auth attempt (browser tab closed, bot probe) adds a
permanent entry. This HashMap grows unboundedly over days.

**Fix:**
Background task in `tasks/` running every 5 minutes:
```rust
let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
let mut nonces = state.siws_nonces.lock().await;
nonces.retain(|_, (_, expires)| *expires > now);
```

---

## Gap 17 — Grafana deployed with default admin/admin credentials

**Severity: High**

`docker-compose.yml:63-64`:
```yaml
- GF_SECURITY_ADMIN_USER=admin
- GF_SECURITY_ADMIN_PASSWORD=admin
```

If the monitoring stack is reachable from the internet even briefly, the
Grafana instance is trivially compromised. From Grafana an attacker can read
all metrics and pivot to backend config via the Prometheus data source.

**Fix:**
```yaml
- GF_SECURITY_ADMIN_PASSWORD=${GRAFANA_ADMIN_PASSWORD}
```
Set `GRAFANA_ADMIN_PASSWORD` to a strong random value in `.env` (gitignored).
Bind Grafana to `127.0.0.1` only and access via SSH tunnel.

---

## Gap 18 — Missing security response headers

**Severity: Medium**

The router sets no `X-Frame-Options`, `X-Content-Type-Options`,
`Content-Security-Policy`, or `Strict-Transport-Security` headers. Browsers
get no protection against clickjacking or MIME sniffing.

**Fix:**
```rust
use tower_http::set_header::SetResponseHeaderLayer;

.layer(SetResponseHeaderLayer::overriding(
    header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"),
))
.layer(SetResponseHeaderLayer::overriding(
    header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"),
))
.layer(SetResponseHeaderLayer::overriding(
    HeaderName::from_static("strict-transport-security"),
    HeaderValue::from_static("max-age=31536000; includeSubDomains"),
))
```

---

## Gap 19 — Auth signature replay window is 5 minutes

**Severity: Medium**

The SIWS timestamp check allows a 300-second window. A captured auth request
can be replayed for up to 5 minutes.

**Fix:**
Tighten to 60 seconds and track used nonces to prevent replay within the
window:
```rust
if now - payload.timestamp > 60 {
    return Err(AuthError::ExpiredTimestamp);
}
if nonces.contains_key(&payload.nonce) {
    return Err(AuthError::NonceAlreadyUsed);
}
```

---

## Full Priority Table (All Gaps)

| Priority | Gap | Effort |
|---|---|---|
| **P0** | Gap 1: Persist tournament state | 1 day |
| **P0** | Gap 2: Automated DB backup | 2 hours |
| **P0** | Gap 3: Alerting + Slack webhook | 4 hours |
| **P0** | Gap 12: Rotate + remove hardcoded Helius key | 30 min |
| **P0** | Gap 15: Keypair fallbacks → fail fast | 1 hour |
| **P1** | Gap 4: Solana RPC fallback | 2 hours |
| **P1** | Gap 5: Graceful shutdown drain | 2 hours |
| **P1** | Gap 7: Global rate limiting | 2 hours |
| **P1** | Gap 13: Restrictive CORS | 1 hour |
| **P1** | Gap 14: Request body size limit | 30 min |
| **P1** | Gap 17: Grafana credentials | 30 min |
| **P2** | Gap 6: Load test + ceiling | 1 day |
| **P2** | Gap 8: Fee payer alert | 2 hours |
| **P2** | Gap 16: SIWS nonce cleanup | 2 hours |
| **P2** | Gap 18: Security response headers | 2 hours |
| **P2** | Gap 19: Auth replay window | 1 hour |
| **P3** | Gap 9: TLS documentation | 1 hour |
| **P3** | Gap 10: Devnet hardcodes | 1 hour |
| **P3** | Gap 11: GDPR endpoints | 2 days |
