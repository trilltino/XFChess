# backend/src/signing/solana

Solana instruction builders and RPC helpers for the `xfchess-game` program: recording
ER moves, undelegating games, finalizing (payout), and profile verification.

## Files

| File | Contents |
|------|----------|
| [transactions.rs](transactions.rs) | Instruction/transaction builders (unsigned) |
| [rpc.rs](rpc.rs) | RPC clients for base layer and MagicBlock ER |
| [routing.rs](routing.rs) | Base-vs-ER routing decisions (mirror of the ADR-0002 model) |
| [telemetry.rs](telemetry.rs) | Per-transaction Prometheus metrics |
| [debug.rs](debug.rs) | Backing logic for `GET /api/debug/transaction/:signature` |

## Example

```
routes/main.rs (POST /move/record)
  └─► transactions.rs builds record_move ix
        └─► routing.rs: game delegated? → ER endpoint : base RPC
              └─► rpc.rs submits; telemetry.rs records outcome
```

## Invariants

- Builders return unsigned (or session-co-signed) transactions; player keys never
  appear here.
- Cluster choice always goes through [routing.rs](routing.rs) — an ER write sent to
  base RPC fails with an owner mismatch (see [MAGICBLOCK.md](../../../../MAGICBLOCK.md)).
