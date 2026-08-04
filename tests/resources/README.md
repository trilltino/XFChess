# tests/resources

Tests for the game client's ECS **resources** (`xfchess::game::resources` and the
`ChessEngine`). Compiled through [../lib.rs](../lib.rs); conventions in
[../README.md](../README.md).

## Files

| File | Covers |
|------|--------|
| [engine_tests.rs](engine_tests.rs) | `ChessEngine` legal-move generation (remember `rebuild_legal_move_cache()`) |
| [captured_tests.rs](captured_tests.rs) | `CapturedPieces` material counting |
| [turn_tests.rs](turn_tests.rs) | `CurrentTurn` flow |
| [history_tests.rs](history_tests.rs) | Move history resources |

## Example

```bash
cargo test -p xfchess --lib resources
```
