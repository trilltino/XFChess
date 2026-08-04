# moves_ix

Move instructions record compact board transitions for active games.

## Invariants

- `record` is the Anchor adapter: account constraints, session expiry, and event emission.
- `apply` owns deterministic state mutation: turn, nonce, move count, fee accounting, board validation, terminal result detection, and move timestamps.
- `parent_nonce`, when supplied, must equal the current game nonce before applying the move.
- Move timestamps update both `updated_at` and `last_move_timestamp`.
