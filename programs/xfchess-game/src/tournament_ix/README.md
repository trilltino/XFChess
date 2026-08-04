# tournament_ix

Tournament instructions manage lifecycle, registration, matches, prizes, and tournament-scoped session keys.

## Subdirectories

| Path | Instructions |
|------|--------------|
| [lifecycle/](lifecycle/) | `initialize`, `initialize_escrow`, `initialize_shards`, `start`, `cancel`, `close_tournament` |
| [registration/](registration/) | `register`, `leave` |
| [matches/](matches/) | `initialize_match`, `record_result`, `record_swiss_result` (+ shared [guards.rs](matches/guards.rs)) |
| [prizes/](prizes/) | `fund_prize`, `fund_sol_prize`, `claim_prize`, `distribute` (permissionless crank), share math in [ledger.rs](prizes/ledger.rs) |
| [session/](session/) | `authorize_tournament_session`, `session_create_game`, `session_join_game` |
| [shards.rs](shards.rs) | Player-list shard operations shared by the groups above |

## Invariants

- `shards` owns player-list operations: required shard count, duplicate checks, push, remove, collection, and Swiss standing initialization.
- Registration and leave update both `num_registered_players` and `player_count`.
- Leave removes active vector entries with `Vec::remove`; it must not write default pubkeys into active slots.
- Match results must name the two stored match participants.
- Prize payouts use `prizes::ledger` for place lookup, bit flags, and share math.
- `close_tournament` only closes `Completed` tournaments after all funded prize places are already claimed.
