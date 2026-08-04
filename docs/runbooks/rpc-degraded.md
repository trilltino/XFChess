# Solana / Triton RPC degraded

**Symptom:** slow RPC reads, `solana_rpc` health check failing, settlement/auth latency up,
"primary circuit breaker OPEN" in logs.
**Severity:** S2 (degraded; failover should absorb) → S1 if fallback also down.
**Dashboards:** Grafana → RPC latency/error panel; `journalctl | grep '\[rpc\]'`.

## Background
Primary RPC = **Triton One** (`SOLANA_RPC_URL`, rpcpool, x-token in URL — redacted in logs).
Fallback = `SOLANA_RPC_FALLBACK_URL` (public devnet). `read_with_failover` + a circuit
breaker (3 fails → 30s cooldown) route reads to the fallback automatically. Every client
has a 30s timeout.

## Diagnose
1. Which endpoint is failing? Logs show redacted host + "failing over to ...".
2. Triton status: check the Triton/rpcpool dashboard (rate-limit tier = developer — are we
   throttled? 429s?).
3. Fallback (public devnet) healthy? `curl -s https://api.devnet.solana.com -X POST -d
   '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' -H 'content-type: application/json'`.

## Mitigate
1. Triton throttled → the breaker already sheds to fallback; consider upgrading the Triton
   tier or adding a second keyed provider (Helius) as an additional fallback.
2. Both degraded → reads will be slow; non-critical features degrade. Communicate status.
3. Rotate a leaked/abused x-token in the Triton dashboard and update `.env`; restart.

## Verify
1. `/health/detailed` → `solana_rpc` ok; breaker closes (no more OPEN logs); latency normal.

## Root cause / follow-up
- Persistent Triton issues → formalize multi-provider failover ordering in WS-D config and
  record provider SLAs in a VENDORS.md (not yet written).
