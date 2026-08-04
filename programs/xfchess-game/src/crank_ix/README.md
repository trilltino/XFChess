# crank_ix

MagicBlock scheduled-task ("crank") instructions that enforce chess time controls on
the Ephemeral Rollup without external infrastructure. Feature-gated behind `cranks`
(on by default; pulls in `magicblock-magic-program-api` + `bincode`).

**Live since the time-check crank wiring**: the backend calls `schedule_time_check`
right after `delegate_game` (`backend/src/signing/routes/main.rs`'s `delegate_game`
handler) and `cancel_time_check` alongside every `undelegate_game` call (both the
`undelegate_game` HTTP handler and the settlement worker's auto-undelegate path in
`backend/src/tasks/settlement_worker.rs`). Before this, `crank_time_check` existed
but nothing ever called it — and separately, nothing called the permissionless
`claim_timeout` instruction either, so a stalled clock just sat `Active` forever.

## Files

| File | Instruction |
|------|-------------|
| [schedule_time_check.rs](schedule_time_check.rs) | `schedule_time_check_crank` — registers a recurring ER task (`task_id`, `check_interval_millis`, `iterations`; 0 = until cancelled) via `crate::magicblock::crank::build_time_check_schedule_instruction` |
| [crank_time_check.rs](crank_time_check.rs) | `crank_time_check` — the callback the ER invokes each interval; flags the game if a clock expired |
| [cancel_time_check.rs](cancel_time_check.rs) | `cancel_time_check_crank` — stops a previously scheduled task via `crate::magicblock::crank::build_time_check_cancel_instruction` (MagicBlock's `CancelTask`); must be signed by the same payer that scheduled it |

## Example

```rust
// crank_time_check.rs — the entire callback body
let game = &mut ctx.accounts.game;
let now = Clock::get()?.unix_timestamp;
crate::lifecycle::terminal::finish_by_timeout_if_expired(game, now)?;
```

## Invariants

- `schedule_time_check_crank` and `cancel_time_check_crank` must both be sent to the
  **Ephemeral Rollup**, not the base layer — both CPI into `MAGIC_PROGRAM_ID`.
- Timeout resolution goes through `crate::lifecycle::terminal::finish_by_timeout_if_expired`;
  the crank never mutates game status directly, so repeated firings are idempotent.
- `cancel_time_check_crank`'s cancelling signer must match the original scheduler's
  payer — MagicBlock's `CancelTask` requires the task authority to match. The backend
  always uses the same session/VPS keypair for both calls on a given game, so this is
  automatic in practice.
- Tournament round advancement and prize distribution are **not** cranked here — the
  backend does that off-chain (`backend/src/tasks/tournament_scheduler.rs`).

## What this does *not* fix

This crank enforces chess-clock timeouts. It has no bearing on the separate,
still-open gap where a delegated `Game` PDA can't be forced back to the base layer
if the ER validator itself is unreachable (see root `MAGICBLOCK.md`'s "Failure Mode:
ER Unavailability" section) — that requires a MagicBlock delegation-program admin
capability XFChess doesn't hold, and no on-chain crank can work around it since
write-authority over a delegated account belongs to the delegation program, not to
any signer of ours.
