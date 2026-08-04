# ADR 0003: Canonical Game Settlement

## Status

Accepted.

## Decision

Game-ending instructions may record terminal results, but final payout and profile/stat mutation belong in the settlement path.

## Consequences

- Resign and timeout are smaller and easier to reason about.
- Dispute resolution can validate a result without hand-writing escrow payout logic.
- Settlement tests can cover fund conservation and profile updates in one place.
