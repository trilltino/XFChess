# ADR 0002: Magic Router Routing Boundary

## Status

Accepted.

## Context

Delegated and base accounts can require different blockhash and routing behavior. Backend or client logic that guesses base RPC versus ER RPC from a local boolean can drift from actual delegation state.

## Decision

Transaction submission should move behind a MagicBlock routing adapter. The adapter should prefer Magic Router for game lifecycle and move transactions, with base-RPC fallback only for transactions that are known to write no delegated accounts.

The current implementation adds backend and native-client routing facades. Backend move submission and undelegation use `MAGIC_ROUTER_RPC_URL` / `MAGIC_ROUTER_URL` when set, falling back to `ER_RPC_URL`; base settlement uses the base Solana RPC.

## Consequences

- Client and backend code stop duplicating routing decisions.
- Delegation state sync can use MagicBlock state as authority, with `Game::is_delegated` as a program mirror.
- The crank scheduling facade can be upgraded separately from lifecycle semantics.
