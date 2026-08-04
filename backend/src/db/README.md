# backend/src/db

SQLite persistence layer (SQLx 0.8, WAL mode). Two schema sources coexist: numbered
migrations in [../../migrations/](../../migrations/) (001–019: KYC, auth, disputes,
anticheat, friends, puzzles, job queue, …) and the runtime `create_tables` bootstrap
in [schema.rs](schema.rs) for the games/moves feed tables.

## Role in XFChess

Everything the backend must remember across restarts lives here: finished-game
archive (PGN, final FEN, settlement signature), per-move records for the spectator
feed, and resumable player sessions. Live tournament state is stored separately as a
JSON blob per record by [../signing/storage/tournament.rs](../signing/storage/tournament.rs)
— not as normalized tables.

## Key files

| File | Contents |
|------|----------|
| [schema.rs](schema.rs) | `init_db` — pool setup (WAL, 30s busy timeout) + games/moves table bootstrap |
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

- Schema changes go in a **new** numbered migration file; never edit an existing one.
- PGN text may be stored raw or `zstd:`-prefixed compressed — always read through
  `decompress_pgn`, never assume plain text.
- Queries use runtime `sqlx::query` (no compile-time `query!` macros), so there is no
  `DATABASE_URL`/offline-cache requirement at build time.
