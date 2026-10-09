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

## Game PDA missing
`finalize_game` closes the Game PDA (`close = fee_payer`), so a missing account is normally
a settled game. The worker only retires such a session when the session is older than 10
minutes **and** the wager escrow PDA (`["escrow", game_id]`) holds at most dust (≤ 5,000
lamports). Otherwise it keeps the session and logs:

`game N: Game PDA missing but escrow still holds X lamports — operator recovery required`

That line means a stake is held with no Game account to settle it. Do not transfer the
escrow by hand. Pull the escrow's signature history, find the last `finalize_game`/
`cancel_game` for the game, preserve it, and escalate (program-authority recovery).
RPC failures leave the session untouched; they never count as "missing".

## Verify
1. Winner balance increases; Game PDA shows finalized; settlement lag returns to < 2 min SLO.

## Root cause / follow-up
- If caused by lost worker state on crash → prioritize the durable job queue (WS-A, P1).
