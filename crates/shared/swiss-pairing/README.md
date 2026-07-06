# swiss-pairing

A FIDE Dutch System Swiss tournament pairing engine with color balancing, used by the
backend's tournament engine to pair each round.

## Modules

| Module | Contents |
|--------|----------|
| `pairing.rs` | Round pairing: scoregroup formation, top/bottom-half split, transposition and exchange handling per the Dutch system |
| `standings.rs` | Standings and tiebreaks (points, Buchholz, Sonneborn-Berger) |
| `color.rs` | Color-preference rules: absolute/strong/mild preferences and balancing across rounds |
| `types.rs` | `Player`, pairing results, round state |
| `error.rs` | Pairing failure modes (e.g. no legal pairing available) |

## Features

- **default** — pure pairing logic, no I/O. Safe anywhere.
- **`network`** — adds Axum route handlers exposing pairing over HTTP. **Backend
  only**; never enable this from the game client.

## Usage

The backend calls into this crate when advancing a Swiss tournament round
(`backend/src/tasks/` auto-advancement and the tournament routes). Pairing is
deterministic for a given standings input, which keeps rounds reproducible and
auditable against the on-chain `record_swiss_result` history.
