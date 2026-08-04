# Caching & Capacity — XFChess

What is cached where (with TTLs), where the bottlenecks are, and how to measure.
Part of the [Production Reality Plan](PRODUCTION_REALITY_PLAN.md) WS-C. Checklist §9, §19.

## Cache inventory (checklist: "contents, location, TTLs documented")

| Cache | Location | Contents | TTL | Miss behaviour | Stale-data risk |
|---|---|---|---|---|---|
| `RateCache` | in-process, [rates.rs](../backend/src/signing/routes/rates.rs) | SOL→USD/GBP/EUR/CAD/BRL rates (Helius → CoinGecko fallback + Frankfurter FX) | 60s | fetch; **serves stale on fetch failure** rather than 503 | wager tier display drift ≤ 60s — acceptable; settlement amounts are on-chain, not rate-derived |
| `EloCache` | in-process, [elo_cache.rs](../backend/src/signing/elo_cache.rs) | on-chain `PlayerProfile` (ELO, RD, username, country, lichess flags) keyed by wallet | constructor-set (matchmaking path) | RPC `get_account` via hardened `make_rpc` (30s timeout) | matchmaking pairs on slightly stale ELO — acceptable; has `invalidate()` for post-game updates |
| RPC circuit breaker | in-process, [rpc.rs](../backend/src/signing/solana/rpc.rs) | primary-endpoint health (not data) | 30s cooldown | fail over to `SOLANA_RPC_FALLBACK_URL` | none (health state only) |
| nginx static | VPS | web bundle assets | file mtime | disk | none (immutable hashed filenames) |

**Design decision (WS-C, revised on implementation):** a generic moka cache layer was
planned, but the hot paths already have purpose-built caches with the right semantics
(stale-on-error for rates, explicit invalidation for ELO). Adding moka would duplicate
them — skipped until a new hot read appears that neither cache covers. No cache tenant
isolation is needed: entries are keyed by wallet/currency and contain no cross-user data.

**Stampede posture:** `RateCache` serves stale on failure (a cold-start thundering herd can
issue a few duplicate upstream fetches — harmless at our QPS). `EloCache` misses are
per-wallet, so herds don't converge on one key. Revisit with single-flight if load tests
show duplicate-fetch storms.

## Bottleneck map (single Hetzner VPS)

1. **Solana RPC** — every profile read/settlement scan is an RPC round-trip.
   Mitigations: EloCache, Triton primary + fallback + breaker, 30s timeouts.
   At 10×: raise Triton tier; add a second keyed provider.
2. **SQLite (WAL)** — single-writer. Fine at current scale; watch `busy_timeout` errors.
   At 10×: move hot writes behind the job queue (already done for email), then Postgres.
3. **CPU: anti-cheat Stockfish workers** — bounded worker pool; backlog is visible in
   metrics. At 10×: raise worker count or offload to a second box.
4. **P2P relay (Iroh)** — per-game QUIC sessions; memory-light. Not the near-term limit.

## Measuring (do before launch)

- **Load test:** `cargo run -p er-cu-benchmark --bin triton-bench -- read-load`
  (`just viz-bench`) for RPC; add an HTTP load pass (e.g. `oha`/`wrk` against
  `/api/rates/all`, `/health/detailed`, auth flow) from a second machine.
- Record p50/p95/p99 into this file per flow, against the targets in [SLO.md](SLO.md).

| Date | Flow | p50 | p95 | p99 | Max sustainable QPS | Notes |
|---|---|---|---|---|---|---|
| _pending first load test_ | | | | | | |

## Capacity at 10× (projection to validate with the load test)

- API reads: nginx + Axum on 1 vCPU handles O(10³) RPS for cached reads — not the limit.
- RPC-bound flows: limited by Triton tier (developer) — **the** knob to turn first.
- SQLite: thousands of writes/sec in WAL mode; our write rate (games, sessions) is far
  below that. Revisit at sustained >100 writes/sec.
