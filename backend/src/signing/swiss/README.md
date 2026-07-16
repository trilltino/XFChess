# backend/src/signing/swiss

Swiss-system tournament orchestration: drives round pairing/starts over the
[`swiss-pairing`](../../../../crates/shared/swiss-pairing/) crate (FIDE Dutch
algorithm) and pushes pairings to clients in real time.

## Files

| File | Contents |
|------|----------|
| [orchestrator.rs](orchestrator.rs) | Round lifecycle: collect results → pair next round → publish |
| [service.rs](service.rs) | Integration with `TournamentStore` and the on-chain `record_swiss_result` |
| [handlers.rs](handlers.rs) | HTTP endpoints |
| [SCORING.md](SCORING.md) | Scoring and tiebreak rules (points, Buchholz) — read this before touching results math |

## Invariants

- Pairing logic itself lives in the `swiss-pairing` crate; this module only
  orchestrates. Algorithm fixes go there, with tests.
- Standings must round-trip with the on-chain `SwissStanding` records —
  [SCORING.md](SCORING.md) defines the shared semantics.
