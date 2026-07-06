# ADR 0001: Split Terminal Result From Settlement

## Status

Accepted.

## Context

MagicBlock delegated accounts are writable on the ephemeral rollup and locked from base-layer writes. Mixed writable transactions with delegated and undelegated accounts are fragile and can fail routing.

XFChess currently has terminal paths that both record results and move SOL/profile state. That creates different settlement behavior for checkmate, resignation, timeout, and finalize.

## Decision

Terminal instructions should only mutate `Game` and record `GameResult`. Settlement should happen once, on base, after final commit and undelegation.

The implementation moves payout/profile logic into `lifecycle::settlement` and makes `resign`/`claim_timeout` terminal-result-only.

## Consequences

- ER instructions can remain `Game`-only.
- Settlement behavior becomes testable as one ledger.
- Backends must wait for undelegation before settlement.
