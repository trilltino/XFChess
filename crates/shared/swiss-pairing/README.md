# swiss-pairing

A FIDE Dutch System Swiss tournament pairing engine with color balancing, used by the
backend's tournament engine to pair each round.

## Modules

| Module | Contents |
|--------|----------|
| `pairing.rs` | Round pairing: scoregroup formation, top/bottom-half split, greedy first-valid-match assignment with float-down for players a scoregroup can't pair internally. Simpler than full FIDE Dutch: no transposition/exchange search over bracket permutations (Handbook C.04.3), so it can float a player in adversarial score distributions where a compliant engine would find a full pairing by reordering. |
| `standings.rs` | Standings and tiebreaks (points, Buchholz, Sonneborn-Berger) |
| `color.rs` | Color-preference rules: balance-based swap decision plus 3-in-a-row/double-same-color checks. Called from `pairing.rs::assign_colors_dutch` — this is the crate's single source of truth for color assignment. |
| `types.rs` | `Player`, pairing results, round state |
| `error.rs` | Pairing failure modes (e.g. no legal pairing available) |

## Features

- **default** — pure pairing logic, no I/O. Safe anywhere. There is currently no
  `network`/HTTP-facing feature — pairing over HTTP, if ever added, is the
  backend's job to wire up in its own routes, not this crate's.

## Usage

The backend calls into this crate when advancing a Swiss tournament round
(`backend/src/tasks/` auto-advancement and the tournament routes). Pairing is
deterministic for a given standings input, which keeps rounds reproducible and
auditable against the on-chain `record_swiss_result` history.
