# ADR 0004: Tournament Shard Invariants

## Status

Accepted.

## Decision

Tournament player shard operations go through `tournament_ix::shards`.

## Consequences

- Registration, leave, start, and Swiss setup share player-list semantics.
- Active vectors never contain default pubkeys as placeholders.
- `players` and `player_elos` lengths are checked before collection or mutation.
- Small, medium, and large tournaments can share the same capacity logic.
