# Swiss Scoring Systems

XFChess uses two scoring representations that must be kept in sync.

## On-chain (Solana contract)

The smart contract stores scores as integers to avoid floating-point on-chain.

| Outcome | Points |
|---------|--------|
| Win     | 2      |
| Draw    | 1      |
| Loss    | 0      |
| Bye     | 2      |

Source: `programs/xfchess-game/src/tournament_ix/matches/record_swiss_result.rs`

## Backend / pairing engine (`crates/swiss-pairing`)

The pairing engine uses FIDE standard floating-point scores.

| Outcome | Points |
|---------|--------|
| Win     | 1.0    |
| Draw    | 0.5    |
| Loss    | 0.0    |
| Bye     | 1.0    |

Source: `crates/swiss-pairing/src/types.rs` — `MatchResult::white_score()` / `black_score()`

## Conversion

When reading on-chain results into the backend, divide by 2:

```rust
let backend_score = contract_points as f64 / 2.0;
```

When submitting standings to the contract, multiply by 2 and round:

```rust
let contract_points = (backend_score * 2.0).round() as u8;
```

This conversion lives in `backend/src/signing/swiss/service.rs`.
The single helper `fn to_contract_points(score: f64) -> u8` must be used
everywhere — do not inline the arithmetic.
