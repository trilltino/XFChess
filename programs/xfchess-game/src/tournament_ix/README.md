# tournament_ix

Tournament instructions manage lifecycle, registration, matches, prizes, and tournament-scoped session keys.

## Invariants

- `shards` owns player-list operations: required shard count, duplicate checks, push, remove, collection, and Swiss standing initialization.
- Registration and leave update both `num_registered_players` and `player_count`.
- Leave removes active vector entries with `Vec::remove`; it must not write default pubkeys into active slots.
- Match results must name the two stored match participants.
- Prize payouts use `prizes::ledger` for place lookup, bit flags, and share math.
- `close_tournament` only closes `Completed` tournaments after all funded prize places are already claimed.
