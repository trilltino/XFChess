# tests/components

Tests for the game client's ECS **components** (`xfchess::game::components`). Compiled
through [../lib.rs](../lib.rs) — Cargo does not build subdirectories as targets on
their own. Conventions: [../README.md](../README.md).

## Files

| File | Covers |
|------|--------|
| [piece_tests.rs](piece_tests.rs) | `Piece`, `HasMoved`, `SelectedPiece` component behavior |
| [game_state_tests.rs](game_state_tests.rs) | `GamePhase`, `MoveRecord` transitions and data |

## Example

```bash
cargo test -p xfchess --lib components
```
