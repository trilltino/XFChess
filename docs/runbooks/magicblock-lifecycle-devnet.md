# MagicBlock Lifecycle Devnet Runbook

Use this runbook for live MagicBlock validation. The full happy path depends on MagicBlock delegation, asynchronous commit, and undelegation behavior that local program-test does not reproduce.

Use the toolchain/dependency pins in [Cargo.toml](../../Cargo.toml) and
[the Anchor builder](../../docker/anchor-builder.Dockerfile). Record cluster,
program bytes, commit and submitted signatures with each drill. Program-test
does not prove a live asynchronous commit/undelegation flow.

## Flow

1. Create a PvP game on base.
2. Join the game on base.
3. Authorize the session-key model used by both players (per-game or global).
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

Live-devnet drills live in
[chain/recovery.rs](../../tools/cu_benchmark/src/chain/recovery.rs), with CLI
wiring in [live/recovery.rs](../../tools/cu_benchmark/src/cli/live/recovery.rs).
Check the runner's `--help` before selecting live recovery/crank flags.

- `run_stuck_delegation_drill` — the full `request_force_undelegate` →
  `force_undelegate_after_timeout` → `recover_stuck_delegation` chain.
  ~60-70 minutes (dominated by the undelegation-request timeout window).
- `run_crank_liveness_drill` — MagicBlock scheduled-crank liveness. ~1-2 minutes.

Both are opt-in, use funded wallets and are not ordinary CI checks. Retain the
result, source commit and signatures with the release evidence:

- **Cadence:** monthly, or before any change touching `delegation_ix/`,
  `governance_ix/recover_stuck_delegation.rs`, or `backend/src/tasks/settlement_worker.rs`.
- **Before a mainnet deploy:** run both drills once, unconditionally, regardless
  of the monthly cadence.
- **On failure:** treat as a mainnet blocker, same severity as a failed money-seam test.

See [release verification](../development/verification.md) for the failure
matrix. A runbook's existence does not establish that a drill has passed.
