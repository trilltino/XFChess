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

A bye is reported as its own board slot after the round's real boards
(`SwissMatchResult::Bye`, the bye player passed as both `white` and `black`), so
on-chain scores match the backend's exactly.

Source: `programs/xfchess-game/src/tournament_ix/matches/record_swiss_result.rs`

## Backend / pairing engine (`libs/tournament/swiss-pairing`)

The pairing engine uses FIDE standard floating-point scores.

| Outcome | Points |
|---------|--------|
| Win     | 1.0    |
| Draw    | 0.5    |
| Loss    | 0.0    |
| Bye     | 1.0    |

Source: `libs/tournament/swiss-pairing/src/types.rs` — `MatchResult::white_score()` / `black_score()`

## Tiebreaks and final places

Tiebreaks (Buchholz, then Sonneborn-Berger, then rating) need every player's full
opponent history, which only the pairing engine has. They are computed off-chain
(`calculate_standings`) and are the order the backend shows.

`complete_swiss_tournament` takes the backend's top 10 as an argument. The program
accepts it only if the players are distinct and registered, on-chain scores never
increase down the list, and no unlisted player outscores the last listed one. The
on-chain `SwissStanding.buchholz`/`sonneborn` fields are no longer written.

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
