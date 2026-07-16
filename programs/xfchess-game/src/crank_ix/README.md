# crank_ix

MagicBlock scheduled-task ("crank") instructions that enforce chess time controls on
the Ephemeral Rollup without external infrastructure. Feature-gated behind `cranks`
(on by default; pulls in `magicblock-magic-program-api` + `bincode`).

## Files

| File | Instruction |
|------|-------------|
| [schedule_time_check.rs](schedule_time_check.rs) | `schedule_time_check_crank` — registers a recurring ER task (`task_id`, `check_interval_millis`, `iterations`; 0 = until cancelled) via `crate::magicblock::crank::build_time_check_schedule_instruction` |
| [crank_time_check.rs](crank_time_check.rs) | `crank_time_check` — the callback the ER invokes each interval; flags the game if a clock expired |

## Example

```rust
// crank_time_check.rs — the entire callback body
let game = &mut ctx.accounts.game;
let now = Clock::get()?.unix_timestamp;
crate::lifecycle::terminal::finish_by_timeout_if_expired(game, now)?;
```

## Invariants

- `schedule_time_check_crank` must be sent to the **Ephemeral Rollup**, not the base
  layer — the schedule CPI targets `MAGIC_PROGRAM_ID`.
- Timeout resolution goes through `crate::lifecycle::terminal::finish_by_timeout_if_expired`;
  the crank never mutates game status directly, so repeated firings are idempotent.
- Tournament round advancement and prize distribution are **not** cranked here — the
  backend does that off-chain (`backend/src/tasks/tournament_scheduler.rs`).
