# governance_ix

Governance instructions open, resolve, and expire disputes.

## Invariants

- Dispute reason and resolution strings are explicitly length-checked before assignment.
- `resolution::validate_resolution` accepts only `None` for draw or one of the two game players as winner.
- Resolution updates dispute state and the game result/status. Escrow payout should use the same settlement helpers as game settlement paths.
- Stale dispute claims split the full pot, not one side's wager.
