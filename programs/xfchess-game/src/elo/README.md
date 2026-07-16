# elo

On-chain rating math for `PlayerProfile.elo_rating`. All ratings are stored in
**centiscale** (×100): 1200 Elo is stored as `120000`. Called from
`lifecycle/settlement.rs` when a game settles.

## Files

| File | Contents |
|------|----------|
| [glicko2.rs](glicko2.rs) | `calculate_elo_update` — K=32 Elo update for both players, 100-Elo floor (the file name is historical; the formula is plain Elo, not Glicko-2) |
| [rating.rs](rating.rs) | Centiscale conversions and bounds: `external_to_centiscale`, `centiscale_to_display`, `validate_external_rating` (external ratings must be 100–4000) |

## Example

```rust
// lifecycle/settlement.rs — sa is white's score: 1.0 win, 0.5 draw, 0.0 loss
let (new_white, new_black) =
    crate::elo::calculate_elo_update(white_rating as f64, black_rating as f64, sa);
```

## Invariants

- Every rating that crosses this module is centiscale. Convert external (display)
  ratings with `external_to_centiscale` on the way in and `centiscale_to_display` on
  the way out — never mix scales.
- K_SCALED = 3200 and DIVISOR = 40000 are the standard K=32 / 400-point constants
  pre-multiplied by the ×100 scale.
- Ratings never drop below `ELO_FLOOR` (100 Elo = 10000 centiscale).
- Linked external ratings (e.g. Lichess) are validated to 100–4000 before storage
  (`account_ix` external-Elo instructions).
