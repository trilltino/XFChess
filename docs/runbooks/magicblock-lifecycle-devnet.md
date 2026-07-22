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
