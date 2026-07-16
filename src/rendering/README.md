# src/rendering

3D visualization of the chess game: board construction, piece meshes, cameras,
lighting, and move-feedback effects. Purely presentational — it reads game state and
never mutates it.

## Role in XFChess

Consumes `ChessEngine` state and `game/` components and draws them. Piece meshes come
from [`assets/`](../assets/) (`GameAssets.piece_meshes`); quality settings arrive via
`GameSettings` and are applied by [graphics_quality.rs](graphics_quality.rs).

## Layout

| Path | Contents |
|------|----------|
| [board/](board/) | Board mesh + themes ([board_theme.rs](board/board_theme.rs)), square coordinates, TempleOS-style debug UI |
| [pieces/](pieces/) | `Piece`, `PieceColor`, `PieceType` components and piece spawning |
| [camera/](camera/) | `camera_director.rs` — in-game camera rigs and transitions |
| [effects/](effects/) | Check highlight, last-move marker, move hints, dynamic lighting, sky |
| [graphics_quality.rs](graphics_quality.rs) | Applies quality presets (SSAO, bloom) to cameras/lights when `GameSettings.graphics_quality` changes |
| [utils.rs](utils.rs) | `Square` ↔ world-position conversion helpers |

## Example

```rust
// graphics_quality.rs — presets toggle real Bevy 0.18 components
use bevy::pbr::ScreenSpaceAmbientOcclusion;
use bevy::post_process::bloom::Bloom;

// watches GameSettings.graphics_quality and inserts/removes
// ScreenSpaceAmbientOcclusion + Bloom on the game cameras
```

## Gotchas

- Keep this module read-only with respect to game state: effects react to
  `game/` components (`HasMoved`, check state) rather than computing chess logic.
- Board-square ↔ world math must go through [utils.rs](utils.rs) `Square` helpers so
  the isometric layout stays consistent with picking.
