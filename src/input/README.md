# src/input

Pointer interaction for the 3D board: hover feedback, cursor management, and the
bridge from Bevy picking events to piece selection.

## Role in XFChess

Sits between Bevy's built-in picking (part of `DefaultPlugins`) and
[`game/`](../game/): pointer events resolve to board squares/pieces here, then the
`GameSystems::Input` set in `game/systems/input.rs` turns them into selections and
moves.

## Key files

| File | Contents |
|------|----------|
| [pointer.rs](pointer.rs) | Hover effects via `Pointer<Over>` / `Pointer<Out>` observers, `CursorIcon` switching, cursor-position resource |
| [mod.rs](mod.rs) | `InputPlugin` (currently only registers pointer systems — Bevy's `PointerInputPlugin` is already in `DefaultPlugins`) |

## Example

```rust
// pointer.rs — hover state comes from Bevy picking events, gated by game phase
use bevy::picking::events::{Out, Over, Pointer};
use crate::game::resources::{CurrentGamePhase, CurrentTurn, Selection};
use bevy::window::{CursorIcon, SystemCursorIcon};
```

## Gotchas

- Selection logic itself lives in [game/systems/input.rs](../game/systems/input.rs);
  this module only produces hover/cursor state. Don't add move execution here.
- Hover systems check `CurrentGamePhase`/`CurrentTurn` so pieces don't light up when
  it isn't your turn — keep that gating when adding new pointer feedback.
