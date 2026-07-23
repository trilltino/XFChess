# backend-types

Serde-only DTOs shared between the backend and the web frontend (`xfchessdotcom`) over
JSON. The crate is deliberately free of Bevy and any game-client dependency so the
web-facing API surface can evolve without dragging the game toolchain along.

## Modules

| Module | Contents |
|--------|----------|
| `tournament.rs` | The tournament API surface, e.g. `TournamentSummary` — the shape the backend's tournament routes serialize and the web UI renders |

## Rules

- **Serde only.** No Bevy, no Solana SDK, no async runtimes. If a type needs those,
  it belongs in [`shared`](../shared/) (client↔backend) or a more specific crate.
- Field changes here are **API contract changes** for the web frontend — coordinate
  with `xfchessdotcom`'s TypeScript expectations when renaming or removing fields.
