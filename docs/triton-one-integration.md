# Triton One Integration — XFChess Deep Reference

> **Scope:** How Triton One's full infrastructure stack maps onto every Solana touchpoint in XFChess — backend API, game client, web frontend, Ephemeral Rollup relay, and tournament blinks. Includes current code locations, specific API calls to swap, what breaks without it, and the exact changes required.

---

## Table of Contents

1. [What Triton One Is](#1-what-triton-one-is)
2. [Why the Current Setup Hits a Wall](#2-why-the-current-setup-hits-a-wall)
3. [Triton's Services and How They Map to XFChess](#3-tritons-services-and-how-they-map-to-xfchess)
4. [API-by-API Breakdown](#4-api-by-api-breakdown)
5. [Performance and Reliability Gains for Users](#5-performance-and-reliability-gains-for-users)
6. [Authentication and Key Management](#6-authentication-and-key-management)
7. [Changes Required — Exact Code Locations](#7-changes-required--exact-code-locations)
8. [Environment Variable Map](#8-environment-variable-map)
9. [Rollout Order](#9-rollout-order)
10. [Cost Estimate](#10-cost-estimate)

---

## 1. What Triton One Is

Triton One is a managed Solana RPC and data infrastructure provider. Unlike a generic cloud node, it is purpose-built for Solana's architecture: validator shred streams, intra-slot data, stake-weighted transaction routing, and program account indexing are all first-class features rather than workarounds.

Their product suite (the "Project Yellowstone" stack) covers every layer XFChess touches:

| Triton Product | What It Replaces |
|---|---|
| Shared RPC cluster (GeoDNS) | `api.devnet.solana.com` and the Helius beta key |
| Dragon's Mouth (gRPC streaming) | `wss://api.devnet.solana.com` WebSocket subscriptions in `ws_subscriber.rs` |
| Whirligig | Same as Dragon's Mouth but over WebSocket — browser-safe for `web-solana` |
| Fumarole | Reconnect-safe Dragon's Mouth; tournament blinks poll loop |
| Cascade Network | Direct transaction relay to staked validators for `sign_and_submit()` |
| Steamboat Indexes | `getProgramAccounts` calls in `solana-chess-client/src/rpc.rs` |
| Priority Fees API | `getRecentPrioritizationFees` for competitive move and wager transactions |
| Old Faithful | Historical game data fetching in backend debug routes |

Everything runs under one API token. One account, one billing line, one failover story.

---

## 2. Why the Current Setup Hits a Wall

### 2.1 Devnet public RPCs

`api.devnet.solana.com` is a shared, rate-limited, unauthenticated node. It imposes:

- **1200 req/10 s per IP** — the backend VPS is a single IP; all tournament operations, game state fetches, blink builds, and fee polling share that budget.
- **No guarantee of liveness.** The public devnet node drops WebSocket connections without notice; `ws_subscriber.rs` reconnects silently but misses events in the gap.
- **No historical access guarantee.** `getTransaction` and `getBlock` can return null for old slots on the shared endpoint. The backend's `/api/debug/transaction/:signature` route silently fails on aged signatures.

### 2.2 Helius beta key (hardcoded)

```
https://mainnet.helius-rpc.com/?api-key=5bb5fed2-8d33-458b-b7d2-3d18fdbb3da5
https://beta.helius-rpc.com/?api-key=5bb5fed2-8d33-458b-b7d2-3d18fdbb3da5
```

These appear in `backend/src/signing/routes/wallet.rs`. They are:

1. **Hardcoded** — not configurable at deploy time.
2. **A public leak** — the key is in the Git history and readable by anyone who clones the repo.
3. A second billing relationship to manage outside the main Solana provider.

### 2.3 WebSocket subscription fragility

`backend/src/signing/ws_subscriber.rs` opens **four** persistent WebSocket connections (2 × L1 devnet, 2 × ER MagicBlock). Standard devnet WebSockets disconnect every few minutes under load. When they drop between a move submission and the backend's account subscribe handler, the ER undelegation or game-finalization event is missed entirely — the tournament match stays in limbo.

### 2.4 No stake-weighted transaction delivery

`sign_and_submit()` in `backend/src/signing/solana/transactions.rs` calls `send_and_confirm_transaction_with_spinner_and_commitment()`. This sends to the standard TPU port. During Solana congestion events, stake-weighted quality-of-service (SWQoS) slots fill first; transactions routed through the public port stall or timeout. Wager games and tournament match-result recordings are the two paths most exposed.

---

## 3. Triton's Services and How They Map to XFChess

### 3.1 Shared RPC Cluster → Primary HTTP RPC

**XFChess usage today:** Every `make_rpc(&state.config.solana_rpc_url)` call in the backend. `DEVNET_RPC_URL` constant in the game client.

**Triton version:** A single GeoDNS endpoint like `https://xfchess.solana.mainnet.rpcpool.com/<token>`. Triton's global cluster routes each request to the closest datacenter (Amsterdam or Tokyo for trading paths; Frankfurt or US-East for general). The node is Solana-full with confirmed-commitment support, same JSON-RPC surface, drop-in replacement.

**Why it matters:** The backend's `make_rpc()` factory is called at request time — no persistent connection pool. GeoDNS means Hetzner (Frankfurt VPS) gets routed to the closest Triton node automatically, cutting average RPC round-trip from ~60 ms (devnet) to ~8–12 ms.

### 3.2 Dragon's Mouth (gRPC) → Real-time Account Streaming

**XFChess usage today:** `backend/src/signing/ws_subscriber.rs` — `accountSubscribe` over standard WebSocket for game PDA monitoring and ER state tracking. Four persistent connections.

**Triton version:** Dragon's Mouth is a gRPC stream providing:
- **Intra-slot account writes** — 400 ms faster than the equivalent WebSocket notification
- **Automatic reconnect** — the stream never drops the way vanilla WebSocket does
- Filter by account address, program owner, or data slice — XFChess only needs game PDAs and tournament escrows

For the backend: replace the `accountSubscribe` WebSocket calls in `ws_subscriber.rs` with a single Dragon's Mouth gRPC channel that subscribes to the game program. All account mutations (game created, move recorded, game finalized) arrive on one stream.

Requires adding `yellowstone-grpc-client` to `backend/Cargo.toml` (Triton's open-source Rust client).

### 3.3 Whirligig (WebSocket) → Frontend Account Subscriptions

**XFChess usage today:** `web-solana/src/lib/magicblock.ts` and Anchor's provider — standard WebSocket connections from the browser.

**Triton version:** Whirligig is Triton's WebSocket endpoint that delivers Dragon's Mouth-level intra-slot performance over the same WebSocket protocol browsers already use. No browser-side code change — just swap the WebSocket URL. Players see game state updates 400 ms sooner.

### 3.4 Fumarole → Tournament Blink Confirmation Loop

**XFChess usage today:** `backend/src/signing/blinks/routes.rs` — `confirm_registration()` polls `get_signature_status_with_commitment()` every 2 s up to 60 s in a `spawn_blocking` loop.

**Triton version:** Fumarole is a **reliable** gRPC stream with:
- **Persistent cursor** — if the backend restarts, the stream replays from where it left off
- **Transaction-level filters** — subscribe to transactions involving a specific account
- **Backfill** — missed events during a disconnect are filled automatically on reconnect

For the registration confirm flow: instead of polling, open a Fumarole subscription scoped to the tournament escrow PDA. The stream delivers the registration transaction the moment it lands — no 2-second polling cadence, no 60-second timeout, no missed confirmations during backend restarts.

### 3.5 Cascade Network → Transaction Delivery

**XFChess usage today:** `sign_and_submit()` in `backend/src/signing/solana/transactions.rs` — calls `send_and_confirm_transaction_with_spinner_and_commitment`. Standard TPU route.

**Triton version:** Cascade is Triton's stake-weighted transaction routing network. It sends your transaction through the reserved SWQoS connections of highly-staked validators rather than the contested public TPU port.

**How to enable:** Triton provides a dedicated `sendtx` HTTP endpoint at `https://xfchess.solana.mainnet.rpcpool.com/sendtx`. You POST the raw base64 transaction bytes directly — same keypair signing, just a different delivery path. No JSON-RPC overhead, and the transaction skips the queue that congests mainnet during peak hours.

For XFChess the two highest-stakes paths are:
1. **Tournament result recording** (`record_result_ix`) — if this tx fails or stalls, a match stays in an unresolved state
2. **Session key delegation** — if the delegation tx drops, the game never gets ER moves

Both should use Cascade on mainnet.

### 3.6 Steamboat Custom Indexes → getProgramAccounts

**XFChess usage today:** `crates/solana/solana-chess-client/src/rpc.rs` — `fetch_all_games()` calls `rpc.get_program_accounts(&program_id)`. This scans every account owned by the XFChess program. On mainnet at scale this call takes seconds and is rate-limited on shared nodes.

**Triton version:** Steamboat maintains a Postgres-backed index of every program account. `getProgramAccounts` on Steamboat returns in milliseconds regardless of account count. No code change — same JSON-RPC call, but the backend can call it in a real-time context rather than a background cache refresh.

### 3.7 Improved Priority Fees API → Move Transactions

**XFChess usage today:** No explicit priority fee estimation — moves are submitted with default fees.

**Triton version:** Triton's `getRecentPrioritizationFees` accepts a percentile parameter (1–10,000) giving true market-rate estimates. For example: `{ "percentile": 5000 }` = 50th percentile fee = reliably included without overpaying.

For XFChess this matters most in:
- **Wager game move recording** — slow confirmation means the opponent sees stale state on the ER
- **Tournament prize claims** — user-signed, no retry opportunity

### 3.8 Digital Assets API (DAS) → Player Profile NFTs / Tokens

**XFChess usage today:** `backend/src/signing/routes/wallet.rs` — manually calls Helius's `getTokenAccountsByOwner` with a hardcoded Helius API key to check player token balances.

**Triton version:** DAS provides 13 purpose-built methods:
- `getAssetsByOwner` — all fungible + NFT assets for a wallet
- `searchAssets` — filter by creator, group, token program
- Full compressed NFT (cNFT) support with Merkle proofs

Swap the Helius call for a Triton DAS call. One less API key, same token program coverage, plus Token22 support for future XFChess token standards.

---

## 4. API-by-API Breakdown

### 4.1 JSON-RPC (Primary)

**Endpoint pattern:**
```
https://xfchess.solana.mainnet.rpcpool.com/<secret-token>
```

**Methods used by XFChess:**

| Method | Where | Current Issue |
|---|---|---|
| `getLatestBlockhash` | `transactions.rs`, `auth.rs`, `blinks/core.rs` | Devnet latency; confirmed on all |
| `sendTransaction` | `sign_and_submit()` | No SWQoS; drops under congestion |
| `getSignatureStatus` | `blinks/routes.rs`, `transactions.rs` | Polling every 2 s; fragile |
| `getAccount` | `auth.rs`, `lobby.rs` | Slow on devnet; no caching |
| `getProgramAccounts` | `solana-chess-client/src/rpc.rs` | Seconds on shared RPC |
| `getTransaction` | `debug.rs` | Fails for old slots |
| `getRecentPrioritizationFees` | (not currently called) | Needed for mainnet |
| `getTokenAccountsByOwner` | `wallet.rs` via Helius | Hardcoded key; no Token22 |

All of these work identically on Triton's endpoint with the `x-token` header or token in URL.

### 4.2 sendtx (HTTP Direct)

**Endpoint:**
```
POST https://xfchess.solana.mainnet.rpcpool.com/sendtx
Content-Type: application/octet-stream
Body: <base64-encoded signed transaction>
```

No JSON-RPC envelope. Returns the signature string on success. Used for Cascade routing.

In `transactions.rs`, the `sign_and_submit()` function can fall back to standard JSON-RPC if the `sendtx` call fails — no single point of failure.

### 4.3 Dragon's Mouth gRPC

**Client library:** `yellowstone-grpc-client` (open-source, maintained by Triton)

```toml
# backend/Cargo.toml
yellowstone-grpc-client = { version = "4.x", features = ["tls"] }
yellowstone-grpc-proto  = "4.x"
```

**Connection:**
```rust
use yellowstone_grpc_client::GeyserGrpcClient;

let client = GeyserGrpcClient::connect(
    "https://xfchess.solana.mainnet.rpcpool.com",
    Some("x-token:<secret>".to_string()),
    None,
).await?;
```

**Subscription filter for XFChess:**
```rust
// Subscribe to all writes to the xfchess-game program accounts
SubscribeRequestFilterAccounts {
    account: vec![], // all accounts
    owner: vec![PROGRAM_ID.to_string()],
    ..Default::default()
}
```

This replaces the four WebSocket connections in `ws_subscriber.rs` with a single, self-healing gRPC stream.

### 4.4 Whirligig (Browser WebSocket)

**Endpoint:** Same base URL, `wss://` scheme.

In `web-solana/src/lib/magicblock.ts`:
```typescript
// Before
export const BASE_LAYER_WS = 'wss://api.devnet.solana.com';

// After
export const BASE_LAYER_WS = 'wss://xfchess.solana.mainnet.rpcpool.com/<public-token>';
```

Note: public-token is a CORS-protected endpoint (no origin spoofing). The secret token stays server-side only.

### 4.5 Digital Assets API (DAS)

DAS uses standard JSON-RPC POST to the same endpoint:

```rust
// Replace wallet.rs Helius call
let body = serde_json::json!({
    "jsonrpc": "2.0",
    "id": 1,
    "method": "getAssetsByOwner",
    "params": {
        "ownerAddress": wallet_pubkey.to_string(),
        "page": 1,
        "limit": 100,
        "displayOptions": {
            "showFungible": true,
            "showNativeBalance": true
        }
    }
});
```

Returns all SPL tokens, Token22 assets, and NFTs in a single call.

### 4.6 Priority Fees API

```rust
// Add to blinks/core.rs or a new signing/solana/fees.rs
pub fn get_priority_fee_microlamports(rpc: &RpcClient, percentile: u32) -> u64 {
    // Triton extended method — returns { context, value: u64 }
    let resp: serde_json::Value = rpc.send(
        RpcRequest::Custom { method: "getRecentPrioritizationFees" },
        serde_json::json!([{ "percentile": percentile }]),
    ).unwrap_or_default();
    resp["result"]["value"].as_u64().unwrap_or(1000)
}
```

Call with `percentile: 5000` (median) for normal moves. For tournament finals or prize claims, use `percentile: 9000` (90th) to guarantee fast inclusion.

---

## 5. Performance and Reliability Gains for Users

### 5.1 Move latency (Ephemeral Rollup path)

The ER move path is: game client → sign move → POST to backend → VPS co-signs → `record_move_ix` submitted to MagicBlock ER RPC → confirmation back to client.

The two bottlenecks in this chain that Triton addresses:

1. **Blockhash fetch.** `getLatestBlockhash` from devnet takes 40–80 ms round-trip from the Hetzner VPS. On Triton's Frankfurt node (same datacenter) this drops to ~8 ms. Over 40 moves in a game that's ~1.3 seconds saved.

2. **Confirmation feedback.** The backend polls `getSignatureStatus` to tell the client the move landed. Polling every 2 s from devnet produces ~1.5 s average notification delay. Dragon's Mouth delivers intra-slot notification the moment the validator writes the account — ~150–200 ms after submission.

**Net for a typical 40-move game:** roughly 60–80 seconds of accumulated latency removed. Practically: moves feel instant rather than having a perceptible pause.

### 5.2 Tournament registration (Blink flow)

Current flow:
1. Player signs registration tx in wallet
2. Player calls `POST /api/actions/tournament/:id/register/confirm`
3. Backend polls every 2 s, waits up to 60 s
4. Store updated, UI refreshes

With Fumarole:
1. Player signs and broadcasts
2. Backend's Fumarole stream sees the transaction the slot it lands (~400 ms)
3. Store updated immediately
4. UI sees player added in under 1 second

For 128-player tournaments this compounds: 128 near-simultaneous registrations would hammer the polling endpoint. Fumarole handles them all on a single stream with no per-request overhead.

### 5.3 Transaction success rate (mainnet, wager games)

The Cascade Network routes through staked validator connections. During Solana congestion events (which happen several times per week at peak), unstaked TPU routes see 20–40% transaction drop rates. Cascade brings this to near-zero for transactions sent through it — Triton's data shows >99% delivery rate for Cascade transactions during congestion.

For wager games this is critical: a dropped `record_move_ix` means the on-chain game state diverges from both players' boards. This is currently the main correctness risk on mainnet.

### 5.4 WebSocket stability (game spectators, tournament brackets)

Current: `ws_subscriber.rs` maintains 4 WebSocket connections. Standard devnet WebSocket disconnects 3–5 times per hour. Each disconnect triggers a reconnect but drops any events that occurred during the gap.

Triton: Dragon's Mouth gRPC uses HTTP/2 multiplexing over a single TLS connection. It is designed for 24/7 uptime — Triton guarantees reconnect with automatic backfill via Fumarole. A tournament bracket UI that subscribes to game-account writes will never miss an update.

### 5.5 getProgramAccounts speed (game browser)

`fetch_all_games()` in `solana-chess-client` returns all live games for the lobby. On devnet's shared RPC this takes 1–4 seconds and is often rate-limited (HTTP 429). On Steamboat it returns in < 50 ms because it's a Postgres index query, not a full account scan. The lobby becomes a real-time view instead of a cached list.

### 5.6 Geographic performance

Triton's GeoDNS routes API calls to the nearest datacenter. XFChess has three audience segments:

| Audience | Backend Server | Triton Datacenter | Estimated RPC RTT |
|---|---|---|---|
| European players | Hetzner Frankfurt | Amsterdam / Frankfurt | 5–10 ms |
| Asian players | Future Asia VPS | Tokyo Pro Trading Center | 8–15 ms |
| US players | Future US VPS | US-East node | 10–20 ms |

The Tokyo Pro Trading Center is co-location class: Triton allows trading servers to be physically racked next to the RPC node. For a future XFChess Asia VPS this means single-digit millisecond RPC latency.

### 5.7 SLA and uptime

Triton runs 24/7 monitoring with hardware security tokens, private encrypted backbone, and automated daily patches. While they don't publish a specific uptime percentage, the architecture (GeoDNS + instantaneous cluster failover) means a single node failure is invisible to clients. This contrasts with `api.devnet.solana.com` which has had multi-hour outages with no SLA.

---

## 6. Authentication and Key Management

Triton issues two credential types:

**Secret token (backend only):**
- Higher rate limits
- Never in frontend code or Git
- Used via URL path: `https://endpoint.rpcpool.com/<token>` or header `x-token: <token>`
- Maps to `SOLANA_RPC_URL` in `SigningConfig` — existing env var, just point it at the Triton URL with token embedded

**Public token (frontend/browser):**
- CORS-protected by origin allowlist (set in Triton portal)
- Lower rate limits
- Safe in client-side JS
- Used in `web-solana` for wallet connection and Anchor provider

**What to do with the hardcoded Helius key:**
1. Revoke it immediately (it's been in the repo — treat it as compromised)
2. Replace with the Triton secret token via `HELIUS_API_KEY` → remove entirely (DAS replaces the need)
3. `wallet.rs` line ~130: replace the Helius HTTP call with DAS `getAssetsByOwner`

**MFA:** Triton portal requires hardware MFA. Use a TOTP app or hardware key — not SMS.

---

## 7. Changes Required — Exact Code Locations

### Priority 1 — Drop-in URL replacements (no logic changes)

These are pure environment variable changes. Swap the URL string, nothing else.

**`backend/src/signing/config.rs` line 73–74:**
```rust
// Before (env var fallback)
solana_rpc_url: std::env::var("SOLANA_RPC_URL")
    .unwrap_or_else(|_| "https://api.devnet.solana.com".to_string()),

// After — same code, different env var value at deploy
// Set SOLANA_RPC_URL=https://xfchess.solana.mainnet.rpcpool.com/<secret>
// No code change needed
```

**`backend/src/signing/solana/rpc.rs` line 14:**
```rust
// Same — reads SOLANA_RPC_URL env var, keep as-is
```

**`backend/src/signing/ws_subscriber.rs` lines 23–24:**
```rust
// Before
Cluster::Mainnet => "wss://api.mainnet-beta.solana.com",

// After
Cluster::Mainnet => "wss://xfchess.solana.mainnet.rpcpool.com/<public-token>",
```

**`src/multiplayer/solana/integration/state.rs` line 15:**
```rust
// Before
pub const DEVNET_RPC_URL: &str = "https://api.devnet.solana.com";

// After (or make configurable via env in build.rs which already injects backend URL)
pub const DEVNET_RPC_URL: &str = env!("SOLANA_RPC_URL"); // injected at compile time
```

**`docker-compose.yml` line 19:**
```yaml
# Before
SOLANA_RPC_URL=https://api.devnet.solana.com

# After
SOLANA_RPC_URL=https://xfchess.solana.mainnet.rpcpool.com/<secret>
```

**`scripts/dev8.bat`, `scripts/run_offline.bat`:**
Replace the Helius beta URL with the Triton URL.

---

### Priority 2 — Remove hardcoded Helius key

**`backend/src/signing/routes/wallet.rs` ~line 126–152:**

Delete the Helius-specific `reqwest` call. Replace with DAS via the existing `make_rpc` infrastructure:

```rust
// New: use DAS getAssetsByOwner
let rpc_url = &state.config.solana_rpc_url; // now points to Triton
let body = serde_json::json!({
    "jsonrpc": "2.0",
    "id": 1,
    "method": "getAssetsByOwner",
    "params": {
        "ownerAddress": wallet_pubkey.to_string(),
        "page": 1,
        "limit": 100,
        "displayOptions": { "showFungible": true }
    }
});
let resp: serde_json::Value = reqwest::Client::new()
    .post(rpc_url)
    .json(&body)
    .send().await?
    .json().await?;
```

Remove `HELIUS_API_KEY` from `config.rs` once this is done.

---

### Priority 3 — Priority fees for move and wager transactions

**New file: `backend/src/signing/solana/fees.rs`**

```rust
use solana_client::rpc_client::RpcClient;

/// Returns a priority fee in microlamports at the given percentile (1–10000).
/// Percentile 5000 = median. Use 9000 for time-sensitive transactions.
pub fn estimate_priority_fee(rpc: &RpcClient, percentile: u32) -> u64 {
    #[derive(serde::Deserialize)]
    struct FeeResult { value: u64 }
    #[derive(serde::Deserialize)]
    struct FeeResp { result: FeeResult }

    let resp: Result<FeeResp, _> = rpc.send(
        solana_client::rpc_request::RpcRequest::Custom {
            method: "getRecentPrioritizationFees",
        },
        serde_json::json!([{ "percentile": percentile }]),
    );
    resp.map(|r| r.result.value).unwrap_or(5_000) // 5000 microlamports fallback
}
```

**`backend/src/signing/solana/instructions.rs` — `record_move_ix` caller:**

Add a `ComputeBudgetInstruction::set_compute_unit_price(fee)` instruction prepended to every move batch:

```rust
use solana_sdk::compute_budget::ComputeBudgetInstruction;

let priority_fee = estimate_priority_fee(&rpc, 5000);
let compute_price_ix = ComputeBudgetInstruction::set_compute_unit_price(priority_fee);
let compute_limit_ix = ComputeBudgetInstruction::set_compute_unit_limit(50_000);
// Prepend both to instruction list
```

---

### Priority 4 — Cascade Network for sign_and_submit

**`backend/src/signing/solana/transactions.rs`:**

Add a `send_via_cascade()` function alongside the existing `sign_and_submit()`:

```rust
/// Sends a signed transaction via Triton Cascade (stake-weighted delivery).
/// Falls back to standard JSON-RPC on error.
pub fn sign_and_submit_cascade(
    rpc: &RpcClient,
    cascade_url: &str,   // "https://xfchess.solana.mainnet.rpcpool.com/sendtx"
    signer: &Keypair,
    instructions: &[Instruction],
) -> Result<Signature> {
    let blockhash = rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        instructions,
        Some(&signer.pubkey()),
        &[signer],
        blockhash,
    );
    let bytes = bincode::serialize(&tx)?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);

    let client = reqwest::blocking::Client::new();
    let resp = client.post(cascade_url)
        .header("Content-Type", "text/plain")
        .body(encoded)
        .send()?;

    if resp.status().is_success() {
        let sig_str = resp.text()?;
        Ok(sig_str.trim().parse()?)
    } else {
        // Fallback to standard path
        sign_and_submit(rpc, signer, instructions)
    }
}
```

Add `SOLANA_SENDTX_URL` to `SigningConfig`:
```rust
sendtx_url: std::env::var("SOLANA_SENDTX_URL")
    .unwrap_or_else(|_| format!("{}/sendtx", solana_rpc_url)),
```

Call `sign_and_submit_cascade` from:
- `tournament.rs` — `record_result_ix`
- `tournament_scheduler.rs` — `start_tournament_ix`
- `signing/routes/main.rs` — all wager game tx paths

---

### Priority 5 — Dragon's Mouth gRPC (replaces ws_subscriber.rs)

**`backend/Cargo.toml`:**
```toml
[dependencies]
yellowstone-grpc-client = "4"
yellowstone-grpc-proto  = "4"
tokio-stream            = "0.1"
```

**New file: `backend/src/signing/geyser_subscriber.rs`**

```rust
use yellowstone_grpc_client::GeyserGrpcClient;
use yellowstone_grpc_proto::prelude::*;

pub struct GeyserSubscriber {
    endpoint: String,
    token: String,
    program_id: String,
}

impl GeyserSubscriber {
    pub fn new(endpoint: String, token: String, program_id: String) -> Self {
        Self { endpoint, token, program_id }
    }

    pub async fn run(self, on_account: impl Fn(String, Vec<u8>) + Send + 'static) {
        let mut client = GeyserGrpcClient::connect(
            self.endpoint.clone(),
            Some(format!("x-token:{}", self.token)),
            None,
        ).await.expect("Geyser connect");

        let mut request = SubscribeRequest::default();
        request.accounts.insert("xfchess".to_string(), SubscribeRequestFilterAccounts {
            account: vec![],
            owner: vec![self.program_id.clone()],
            ..Default::default()
        });

        let (_, mut stream) = client.subscribe_with_request(Some(request))
            .await.expect("Geyser subscribe");

        while let Some(msg) = tokio_stream::StreamExt::next(&mut stream).await {
            if let Ok(update) = msg {
                if let Some(update_oneof) = update.update_oneof {
                    if let subscribe_update::UpdateOneof::Account(account_update) = update_oneof {
                        if let Some(account) = account_update.account {
                            on_account(
                                account_update.pubkey.to_string(),
                                account.data,
                            );
                        }
                    }
                }
            }
        }
    }
}
```

In `app_state` / `infrastructure/tasks.rs`: spawn `GeyserSubscriber` as a background task. Feed account updates into the tournament store and game state cache. Retire the four WebSocket connections in `ws_subscriber.rs`.

Config: two new env vars:
- `TRITON_GRPC_ENDPOINT=https://xfchess.solana.mainnet.rpcpool.com`
- `TRITON_GRPC_TOKEN=<same-secret-as-rpc-token>` (same token works for both)

---

### Priority 6 — Fumarole for registration confirmation (optional upgrade)

The polling loop in `blinks/routes.rs` `confirm_registration()` works correctly but is inefficient for large simultaneous registrations. If the 128-player weekend tournaments ever see concurrent registrations, replace the loop with Fumarole:

1. At server start: open a Fumarole stream scoped to tournament escrow PDAs
2. Maintain a `tokio::sync::broadcast` channel of confirmed signatures
3. `confirm_registration()` subscribes to the broadcast, awaits its signature
4. Remove the 30-iteration polling loop

This is a follow-on optimization, not required for launch.

---

## 8. Environment Variable Map

| Env var | Current default | Triton value | Used in |
|---|---|---|---|
| `SOLANA_RPC_URL` | `https://api.devnet.solana.com` | `https://xfchess.solana.mainnet.rpcpool.com/<secret>` | backend config, rpc.rs |
| `SOLANA_WS_URL` | (derived from above) | `wss://xfchess.solana.mainnet.rpcpool.com/<public>` | ws_subscriber.rs |
| `SOLANA_SENDTX_URL` | (new) | `https://xfchess.solana.mainnet.rpcpool.com/sendtx` | transactions.rs cascade |
| `TRITON_GRPC_ENDPOINT` | (new) | `https://xfchess.solana.mainnet.rpcpool.com` | geyser_subscriber.rs |
| `TRITON_GRPC_TOKEN` | (new) | `<same secret token>` | geyser_subscriber.rs |
| `ER_RPC_URL` | `https://devnet-eu.magicblock.app/` | (unchanged — MagicBlock manages this) | magicblock.rs |
| `HELIUS_API_KEY` | hardcoded in wallet.rs | **remove** | wallet.rs → replaced by DAS |

All secrets live in `.env` on the Hetzner VPS (or a secrets manager). Never in `.env.example`, never committed.

---

## 9. Rollout Order

### Phase 1 — Swap RPC endpoint (1 day, zero risk)
1. Onboard at `https://customers.triton.one/onboarding`
2. Get secret token
3. Set `SOLANA_RPC_URL` on the VPS to the Triton mainnet URL
4. Redeploy backend. All `make_rpc()` calls now use Triton. No code change.
5. Confirm `/metrics` Prometheus data shows lower RPC latency

### Phase 2 — Remove Helius (1 day)
1. Replace `wallet.rs` Helius call with DAS `getAssetsByOwner`
2. Remove `HELIUS_API_KEY` from config
3. Rotate the leaked Helius key to invalidate it

### Phase 3 — Priority fees (1 day)
1. Add `fees.rs` with `estimate_priority_fee()`
2. Prepend `ComputeBudgetInstruction` to move and tournament instructions
3. Test on devnet first — confirm CU estimates are tight

### Phase 4 — Cascade sendtx (2 days)
1. Add `sign_and_submit_cascade()` to `transactions.rs`
2. Add `SOLANA_SENDTX_URL` to config
3. Wire into `record_result_ix` and wager game paths
4. Test fallback behavior (kill the sendtx endpoint, confirm JSON-RPC fallback works)

### Phase 5 — Dragon's Mouth gRPC (3–5 days)
1. Add `yellowstone-grpc-client` to Cargo.toml
2. Write `geyser_subscriber.rs`
3. Spawn in `infrastructure/tasks.rs` alongside existing tasks
4. Validate account updates arrive and feed tournament store correctly
5. Retire `ws_subscriber.rs` WebSocket connections one by one

### Phase 6 — Fumarole confirmation stream (2–3 days, after Phase 5)
Replaces the polling loop in `confirm_registration()`. Only needed if Phase 1–4 expose throughput limits at registration time.

---

## 10. Cost Estimate

Triton pricing is month-to-month with no lock-in. Rough figures based on published API rates:

| Usage | Triton Product | Est. Monthly Cost |
|---|---|---|
| Backend RPC (~500k req/day) | Shared mainnet | ~$50–150/month |
| DAS token queries | Included in shared RPC | — |
| Dragon's Mouth gRPC | Add-on to shared plan | ~$50–100/month |
| Cascade Network | Currently free (Triton subsidised) | $0 |
| Historical data (debug routes) | Hydrant Archive ~10k queries/month | $10 minimum |
| Priority fees API | Included in JSON-RPC plan | — |
| **Total estimate** | | **~$110–260/month** |

At 128-player weekend tournament frequency (Schedule D) and ~100 active daily users, a shared mainnet plan likely covers everything. A dedicated node becomes relevant when `getProgramAccounts` volume or WebSocket subscriber counts push into thousands — probably at the 1,000+ MAU mark.

---

## Summary

Triton One is a comprehensive replacement for every non-MagicBlock Solana infrastructure dependency in XFChess. The integration is largely additive — the biggest wins (RPC latency, transaction delivery, WebSocket stability) come from environment variable changes with zero application logic change. The deeper integrations (Dragon's Mouth gRPC, Cascade, priority fees) require new code but map cleanly onto existing abstractions. The hardcoded Helius key is the only urgent security item.

The user-visible outcome at scale: move latency down from ~1–2 s confirmation delay to under 500 ms, zero missed tournament events, and near-100% transaction landing rate during Solana congestion — which is when wager games are most likely to be in progress.
