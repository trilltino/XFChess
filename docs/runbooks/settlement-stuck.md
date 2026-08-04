# Wager settlement stuck

**Symptom:** a game has a result on-chain but the winner wasn't paid; `finalize_game`
not landing; settlement lag alert.
**Severity:** S1 (money not moving).
**Dashboards:** Grafana → settlement worker panel; `journalctl -u xfchess-backend | grep settlement`.

## Background
`tasks/settlement_worker.rs` scans active sessions every 30s, reads the Game PDA, and
submits `finalize_game` once a result is committed. Clients never call finalize directly.

## Diagnose
1. Is the worker running? `journalctl -u xfchess-backend | grep -i settlement | tail`.
2. RPC healthy? `curl -s https://$SERVER/health/detailed` → `solana_rpc` check. If failing
   → [rpc-degraded.md](rpc-degraded.md) (failover should kick in via `read_with_failover`).
3. Fee-payer funded? `/health/detailed` → `feepayer_pool`. Empty/low balance blocks submits.
4. Inspect the specific game: `GET /api/debug/tx/{signature}` and the Game PDA on Solscan.

## Mitigate
1. RPC issue → confirm fallback engaged (log: "failing over to ..."); if primary flapping,
   temporarily set `SOLANA_RPC_URL` to a healthy endpoint and restart.
2. Fee payer empty → fund the fee-payer wallet(s); worker retries automatically.
3. Genuinely stuck game → manual finalize via admin (`/admin/...`) or governance dispute path.

## Verify
1. Winner balance increases; Game PDA shows finalized; settlement lag returns to < 2 min SLO.

## Root cause / follow-up
- If caused by lost worker state on crash → prioritize the durable job queue (WS-A, P1).
