# backend-types

Serde-only DTOs for JSON shapes the backend emits over HTTP. Deliberately free of
Bevy, Solana SDK, and async-runtime dependencies so the wire-format types can be
depended on without dragging in a full client toolchain.

## Modules

| Module | Contents |
|--------|----------|
| `tournament.rs` | `TournamentSummary` — the JSON shape returned by the backend's tournament-list endpoint |

## Consumers

- **Game client** (`src/states/tournament_menu.rs`) depends on this crate directly
  and deserializes into `TournamentSummary`.
- **Backend** does not depend on this crate. `backend/src/signing/routes/tournament.rs`
  defines its own `TournamentSummary` struct that must be field-for-field
  compatible by hand (the struct there is `#[derive(Serialize)]` only, with no
  `#[serde(default)]` on `is_private`/`is_tournament` — an omitted field on either
  side breaks deserialization for every response). See that struct's doc comment
  for the specifics.
- **Web frontend** (`xfchessdotcom`) consumes the same JSON shape via its own
  hand-written TypeScript types, not this crate (TypeScript can't depend on a Rust
  crate) — keep those in sync separately when this shape changes.

## Rules

- **Serde only.** No Bevy, no Solana SDK, no async runtimes. If a type needs those,
  it belongs in [`shared`](../shared/) (client↔backend) or a more specific crate.
- Field changes here are wire-format contract changes across three independent
  definitions (this crate, the backend's local struct, and the web frontend's
  TypeScript types) — update all three together.
