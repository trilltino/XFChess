# Shared types & tournament logic

Crates shared across the game client and backend that are neither engine, Solana,
nor networking code.

| Crate | Purpose | Consumers |
|-------|---------|-----------|
| [`swiss-pairing/`](swiss-pairing/) | FIDE Dutch System Swiss pairing engine with color balancing: `pairing.rs` (pair generation, delegates color assignment to `color.rs`), `standings.rs` (scoregroups, Buchholz/Sonneborn tiebreaks), `color.rs` (color-preference rules), `types.rs`/`error.rs`. Simplified vs. full FIDE Dutch — no transposition/exchange bracket search. | Backend tournament engine |
| [`xfchess-anticheat/`](xfchess-anticheat/) | Server-side post-game anti-cheat analysis. `analyse_game` runs a Stockfish subprocess per game (`engine/stockfish`), extracts features (`features/`: accuracy, timing, blur, complexity, screen), scores them (`scoring/`), calibrates against ELO baselines (`elo_baseline.rs`), and emits `AcReport`s (`report/`). Prometheus metrics in `metrics.rs`; tunable thresholds in `config.rs`. | Backend worker tasks |

## Rules of thumb

- `swiss-pairing` and `xfchess-anticheat` are logic crates the backend orchestrates;
  the game client never links them.
- There used to be a `backend-types` crate here for serde-only backend↔web-frontend
  DTOs. It was deleted (2026-07-31) after an audit found its only Rust consumer was
  a game-client `Bevy` resource field that was never actually populated — the real,
  live tournament-summary type is `src/multiplayer/network/vps/tournament.rs`'s
  `TournamentSummary`, kept in sync with the backend by a regression test in
  `backend/src/signing/routes/tournament.rs`. If a genuine backend↔web-frontend
  shared-DTO need comes up again, recreate the crate against that real contract
  rather than reviving the old one from git history.
