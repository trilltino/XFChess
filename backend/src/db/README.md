# backend/src/db

SQLite persistence layer (SQLx 0.9, WAL mode). The schema for these tables lives in
[../../migrations/](../../migrations/README.md).

## Role in XFChess

Everything the backend must remember across restarts lives here: finished-game
archive (PGN, final FEN, settlement signature), per-move records for the spectator
feed, and resumable player sessions. Live tournament state is stored separately as a
JSON blob per record by [../signing/storage/tournament.rs](../signing/storage/tournament.rs)
— not as normalized tables.

## Key files

| File | Contents |
|------|----------|
| [repository.rs](repository.rs) | Game/move records; zstd+base64 PGN compression; `filter_visible_moves` (anti-cheat broadcast delay) |
| [sessions.rs](sessions.rs) | `SessionStatus` (`Active/Paused/Resumable/Expired`) — disconnect recovery |

## Example

```rust
// repository.rs — the public spectator feed lags live play to deter engine relay
pub fn filter_visible_moves(moves: Vec<MoveRecord>, now_ts: i64, delay_secs: i64) -> Vec<MoveRecord> {
    if delay_secs <= 0 {
        return moves; // casual games stream live
    }
    let horizon = now_ts - delay_secs;
    moves.into_iter().filter(|m| m.timestamp <= horizon).collect()
}
```

## Invariants

- Schema changes follow [../../migrations/README.md](../../migrations/README.md).
- PGN text may be stored raw or `zstd:`-prefixed compressed — always read through
  `decompress_pgn`, never assume plain text.
- Queries use runtime `sqlx::query` (no compile-time `query!` macros), so there is no
  `DATABASE_URL`/offline-cache requirement at build time.
