# Shared types & tournament logic

Crates shared across the game client, backend, and web frontend that are neither
engine, Solana, nor networking code.

| Crate | Purpose | Consumers |
|-------|---------|-----------|
| [`shared/`](shared/) | Client↔backend protocol types (`protocol.rs`) and the CRDT-tagged document types (`crdt.rs`) used by live sync. Depends on Bevy types where needed by the client. | Game client, backend |
| [`backend-types/`](backend-types/) | Serde-only DTOs (currently the tournament API surface in `tournament.rs`) shared between the backend and the web frontend via JSON. Deliberately Bevy-free so the web toolchain never pulls game-client deps. | Backend, `xfchessdotcom` |
| [`swiss-pairing/`](swiss-pairing/) | FIDE Dutch System Swiss pairing engine with color balancing: `pairing.rs` (pair generation), `standings.rs` (scoregroups, Buchholz/Sonneborn tiebreaks), `color.rs` (color-preference rules), `types.rs`/`error.rs`. The `network` feature adds Axum route handlers — backend only. | Backend tournament engine |
| [`xfchess-anticheat/`](xfchess-anticheat/) | Server-side post-game anti-cheat analysis. `analyse_game` runs a Stockfish subprocess per game (`engine/stockfish`), extracts features (`features/`: accuracy, timing, blur, complexity), scores them (`scoring/`), aggregates cross-game evidence (`cross_game/`), calibrates against ELO baselines (`elo_baseline.rs`), and emits `AcReport`s (`report/`). Prometheus metrics in `metrics.rs`; tunable thresholds in `config.rs`. | Backend worker tasks |

## Rules of thumb

- Types consumed by the **web frontend** go in `backend-types` (serde-only).
- Types consumed by the **game client and backend** go in `shared`.
- `swiss-pairing` and `xfchess-anticheat` are logic crates the backend orchestrates;
  the game client never links them.
