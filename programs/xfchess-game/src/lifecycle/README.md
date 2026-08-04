# lifecycle

Plain-Rust game state machine, kept free of Anchor context types so instruction
handlers stay thin adapters and the logic is unit-testable. ADR:
[docs/adr/0001-split-terminal-result-from-settlement.md](../../../../docs/adr/0001-split-terminal-result-from-settlement.md).

## Files

| File | Contents |
|------|----------|
| [transitions.rs](transitions.rs) | Status transitions: `mark_delegated`, `mark_undelegated`, activation |
| [guards.rs](guards.rs) | Preconditions handlers call before mutating (`GamePhase` checks) |
| [terminal.rs](terminal.rs) | Terminal results: resign, draw, `finish_by_timeout_if_expired` |
| [settlement.rs](settlement.rs) | Canonical settlement: pot payout, Elo update, profile stats (ADR-0003) |
| [clock.rs](clock.rs) | Time-control math (base time + Fischer increment) |

## Example

```rust
// crank_ix, game_ix and moves_ix all resolve timeouts through the same function:
crate::lifecycle::terminal::finish_by_timeout_if_expired(game, now)?;
```

## Invariants

- Setting a terminal `GameResult` and paying out are **separate steps**: terminal.rs
  records results; only settlement.rs moves lamports and mutates profiles (ADR-0001,
  ADR-0003).
- Handlers never assign `GameStatus` directly — always through these functions, so
  the allowed-transition rules live in one place.
