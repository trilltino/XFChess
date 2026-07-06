# game_ix

Game instructions create, join, finish, cancel, and settle `Game` accounts.

## Invariants

- `common::init_game_fields` is the only shared initializer for new games.
- `join_game`, `global_join_game`, and tournament session joins must update both `updated_at` and `last_move_timestamp`.
- Result-only endings such as resign and timeout may set `GameStatus::Finished` plus `GameResult`, but final pot payout and profile mutation flow through settlement.
- Wager pot math uses checked helpers from `common::escrow`.

## Lamports

- Create/join paths move player wagers into the wager escrow.
- Final settlement pays from the wager escrow with PDA-signed system transfers.
- Cancel and expired-wager paths refund only the creator-side escrow state they own.
