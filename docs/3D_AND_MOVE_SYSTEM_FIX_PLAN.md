# 3D Rendering & Move System Fix Plan

## 3D Black Screen — Root Cause & Fix

### Root Cause

The 3D board (`assets/models/wooden_chess_board.glb`) embeds textures in JPEG format inside the GLB binary. Bevy 0.18 requires explicit feature flags to enable image decoders; `jpeg` was absent from the workspace dependency declaration.

When Bevy tries to load a GLTF that references embedded JPEG data and the decoder is not compiled in, the entire GLTF scene silently fails to produce any mesh/material entities. The camera and lighting still initialize, so the viewport renders as a solid black rectangle rather than crashing.

### Fix Applied

`Cargo.toml` (workspace root), `bevy` dependency — added `"jpeg"` to the features list:

```toml
bevy = { version = "=0.18.1", default-features = false, features = [
    "default", "webgl2", "mp3", "bevy_gltf", "jpeg"
] }
```

**Rebuild required.** A full `cargo build` (not incremental) is needed for the decoder to be compiled in.

### Verification Steps

1. `cargo build` — must complete without errors
2. Run `cargo run`
3. Enter a game and press `V` to confirm toggle between 2D and 3D works
4. In 3D mode the board should show wood texture; pieces should be visible meshes
5. Check the console for any `Failed to load asset` or `unsupported image format` messages — there should be none related to the GLB

### Secondary Checks (if still black after rebuild)

| Check | How |
|-------|-----|
| Camera has `Camera3d` component | `src/game/systems/camera.rs:826` — confirmed present |
| Lighting spawned | `setup_game_scene` chains after `setup_game_camera` and spawns `DirectionalLight` + ambient |
| No `RenderLayers` mismatch | Pieces and board do not set a custom render layer; camera renders layer 0 by default |
| GLTF scene entity spawned | Add a temporary `info!` in the `SceneRoot` spawn block in `setup_game_scene` |

---

## Camera System Analysis

**File**: `src/game/systems/camera.rs`

### Persistent Camera Pattern

A single camera entity is created once at app startup with marker component `PersistentEguiCamera`. This entity is **never despawned** — it persists across game state transitions.

`setup_game_camera` (line 826) repositions this entity rather than spawning a new one:

```
Query<Entity, With<PersistentEguiCamera>>
  → get or spawn camera entity
  → insert Transform based on ViewMode (2D orthographic vs 3D perspective)
  → insert CameraController
```

### ViewMode Toggle

`view_mode_toggle_input_system` (line 960) handles `V` keypress:
1. Reads current `ViewMode` resource
2. Calls `setup_game_camera` to reposition the same persistent camera entity
3. Updates `ViewMode` resource

### Known Issue

When `setup_game_camera` is called mid-game (toggle), it resets `CameraController` to defaults. Any user-applied orbit/zoom is lost. This is cosmetic but jarring — worth tracking but low priority.

---

## Move System — Full Flow Analysis

### Selection Phase

**Entry point**: `on_piece_click` observer in `src/game/systems/input.rs`

```
Click on piece entity
  → try_select_piece(square)
      → engine.get_legal_moves_for_square(square)  ← nimzovich_engine
      → stores Vec<Move> in SelectionState.possible_moves
      → spawns highlight entities for each valid target square
```

Legal move generation respects:
- Pin detection (absolute pins)
- Check evasion (only moves that resolve check)
- En passant, castling, promotion flags embedded in `Move` bitfield

### Execution Phase

**Entry point**: `on_square_click` observer

```
Click on highlighted square
  → try_move_sequence(from, to)
      → find Move in possible_moves matching (from, to)
      → execute_move(move)
          → update BoardState (FEN + piece positions)
          → emit MoveMadeEvent { move, capture_happened }
          → clear selection + highlights
```

### Post-Move Phase

`MoveMadeEvent` triggers:

| System | Action |
|--------|--------|
| `apply_capture` | Remove captured piece entity; play capture sound |
| `play_move_audio` | Play move sound (skipped if capture) |
| `check_game_over` | Detect checkmate/stalemate; emit GameOverEvent |
| `advance_turn` | Flip active color in BoardState |
| `pending_turn` | If AI turn → dispatch AI move request |
| `sync_3d_pieces` | Reposition 3D mesh entities to new square |
| `sync_2d_pieces` | Reposition 2D sprite entities |

### Audio Logic (subtle)

`play_move_audio` in `src/game/systems/shared.rs`:

```rust
pub fn play_move_audio(commands, move_sound, capture_happened) {
    if capture_happened {
        // capture sound is played in apply_capture — skip here
        return;
    }
    commands.spawn(AudioPlayer::new(move_sound));
}
```

Capture sound is spawned in `apply_capture` (separate system). The guard in `play_move_audio` prevents double-audio. Both systems run in the same frame via `IntoSystemConfigs::chain` ordering.

### Known Move System Issues

#### 1. Promotion — No UI

When a pawn reaches the back rank, `Move` carries a promotion flag but there is no promotion selection popup. The engine defaults to queen. This is a missing feature, not a bug, but results in silent auto-queen.

**Fix**: Intercept in `try_move_sequence` when move is a promotion; pause execution; show promotion UI overlay; resume `execute_move` with user-selected piece type.

#### 2. Network Move Validation Gap

In multiplayer, incoming opponent moves from the WebSocket are applied via `apply_remote_move` which skips the `engine.get_legal_moves_for_square` validation path — it trusts the server. If the backend's validation differs from the local engine's, illegal positions can appear on the client.

**Fix**: Run `engine.is_legal(move, board)` on incoming moves before applying; reject and request resync if illegal.

#### 3. 3D Piece Sync After Camera Toggle

`sync_3d_pieces` uses the current `ViewMode` to decide whether to run. If `ViewMode` changes between move execution and the sync system running in the same frame, pieces may not update their 3D positions.

**Fix**: Decouple `sync_3d_pieces` from `ViewMode` — always sync positions; let the camera/render layer control visibility instead.

#### 4. En Passant Square Not Reset on Illegal Sequence

If a move is selected but then the user clicks an invalid square (deselecting), the engine's internal en passant square is not re-queried. It remains valid until the next legal move generation call. No visible bug today but fragile.

---

## Testing Checklist

### 3D Rendering

- [ ] Board shows wood texture in 3D mode
- [ ] All 32 pieces render as 3D meshes with correct colors
- [ ] `V` key toggles cleanly between 2D and 3D
- [ ] Captured pieces disappear in 3D mode (not just 2D)
- [ ] No console errors about missing assets or unsupported image formats

### Move System — Core

- [ ] Legal moves highlight correctly on piece click
- [ ] Illegal squares are not highlighted
- [ ] Move executes when a highlighted square is clicked
- [ ] Piece position updates in both 2D and 3D after move
- [ ] Active turn advances after each move

### Move System — Special Moves

- [ ] **Castling**: King-side and queen-side both sides; rook teleports to correct square
- [ ] **En passant**: Capturing pawn moves diagonally; captured pawn removed
- [ ] **Promotion**: Pawn reaches back rank; auto-queened (until UI added)
- [ ] **Check**: Opponent's king highlighted (if `check.mp3` plays, bonus)
- [ ] **Checkmate**: Game over screen appears; no further moves possible
- [ ] **Stalemate**: Game over screen appears with draw result

### Move System — Audio

- [ ] Move sound plays on non-capture moves
- [ ] Capture sound plays on captures (not double-played)
- [ ] No audio error in console for missing `check.mp3` / `illegal.mp3`

### Multiplayer

- [ ] Remote moves apply correctly
- [ ] Board stays in sync after 5+ move sequence
- [ ] Disconnect/reconnect does not corrupt board state
