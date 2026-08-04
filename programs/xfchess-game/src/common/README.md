# common

Cross-cutting helpers for instruction handlers. Currently one concern: moving
lamports correctly.

## Files

| File | Contents |
|------|----------|
| [escrow.rs](escrow.rs) | The single source of truth for fund flows: system-owned PDAs (per-game wager escrow) must be debited via **signed CPI `system_program::transfer`**, program-owned PDAs (fee/treasury vaults) via **direct lamport arithmetic** |

## Example

```rust
// game_ix settlement paths pay from the wager escrow via checked helpers here —
// a system-owned escrow may NOT have its lamports decremented directly (the
// runtime rejects it); that distinction caused a real bug and now lives only here.
```

## Invariants

- Any new instruction that moves SOL must call these helpers; do not open-code
  `**account.lamports.borrow_mut() -= x`.
- All pot math uses the checked helpers — overflow returns an error, never wraps.
