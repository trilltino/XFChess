# MagicBlock Lifecycle Devnet Runbook

Use this runbook for live MagicBlock validation. The full happy path depends on MagicBlock delegation, asynchronous commit, and undelegation behavior that local program-test does not reproduce.

## Pinned Toolchain

- Anchor `1.1.2`
- Solana `3.1.12` (Agave)
- `ephemeral-rollups-sdk` `0.16.2`

The Anchor 1.0 / Solana 3.x migration landed 2026-07-22, which also unblocked the `ephemeral-rollups-sdk` bump to `0.16.2` — the version fixing the 2026-07-22 undelegation-buffer disclosure (`undelegate_account()` now checks that `buffer` is the canonical PDA for `delegated_account`, closing the arbitrary-account-overwrite path). Confirmed present in the resolved dependency and covered by `er_delegation_tests.rs`/`er_move_tests.rs`.

## Flow

1. Create a PvP game on base.
2. Join the game on base.
3. Authorize session keys for both players.
4. Call `delegate_game` on base.
5. Submit two or more `record_move` transactions through Magic Router or the ER endpoint used by the backend.
6. Force a terminal result on ER through a legal checkmate/draw path, `resign`, or `claim_timeout`.
7. Call `undelegate_game`.
8. Confirm the base `Game` reflects ER moves, nonce, result, and `is_delegated == false`.
9. Call `finalize_game`.
10. Verify escrow, treasury, player balances, ELO, and stats.

## Failure Checks

- `delegate_game` rejects an already delegated game.
- `undelegate_game` rejects a non-delegated game.
- `finalize_game` and `cancel_game` reject delegated games while they write base-layer money/profile accounts.
- `resign` and `claim_timeout` write only the `Game` PDA and can finish delegated games.
- Backend routing never submits a transaction with mixed writable delegated and undelegated accounts.

## Recovery/liveness drills — rehearsal schedule

Two live-devnet drills exist in `crates/solana/er-cu-benchmark/src/recovery_drill.rs`,
covering the ER-unavailability escape hatch (§1.6, `docs/PRE_MAINNET_E2E_PLAN.md`):

- `run_stuck_delegation_drill` — the full `request_force_undelegate` →
  `force_undelegate_after_timeout` → `recover_stuck_delegation` chain.
  ~60-70 minutes (dominated by the undelegation-request timeout window).
- `run_crank_liveness_drill` — MagicBlock scheduled-crank liveness. ~1-2 minutes.

Both are opt-in and intentionally **not** run in CI (live-devnet, real-time-bound,
cost real devnet SOL/compute). Run them as a scheduled rehearsal instead of ad
hoc, and log every result below rather than discarding the output:

- **Cadence:** monthly, or before any change touching `delegation_ix/`,
  `governance_ix/recover_stuck_delegation.rs`, or `backend/src/tasks/settlement_worker.rs`.
- **Before a mainnet deploy:** run both drills once, unconditionally, regardless
  of the monthly cadence.
- **On failure:** treat as a mainnet blocker, same severity as a failed money-seam test.

### Drill log

| Date | Drill | Result | Notes |
|---|---|---|---|
| _(none yet — first rehearsal not yet run)_ | | | |
