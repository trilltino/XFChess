# XFChess Game Crate Architecture

`programs/xfchess-game` keeps the existing Anchor instruction surface and isolates repeated business rules inside domain-local helper modules.

## Domains

- `account_ix`: profiles, usernames, sessions, fee vaults, and treasury.
- `game_ix`: game creation, joining, result transitions, and base game settlement adapters.
- `moves_ix`: move account validation and pure move bookkeeping.
- `governance_ix`: dispute validation and dispute state transitions.
- `tournament_ix`: tournament lifecycle, shards, matches, prizes, and tournament sessions.
- `elo`: rating math and rating-unit conversions.
- `common`: shared lamport helpers that are not tied to one instruction family.

## Safety Rules

- Game payout and profile/stat mutation are centralized in settlement helpers.
- Escrow pot math is checked.
- Tournament prize destinations are derived from recorded placements.
- Profile initialization is not allowed to reset existing gameplay data.
- Session spending counters use checked arithmetic.
