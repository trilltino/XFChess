# Piece Positioning and Knight Rotation Fix Plan

## Issues Identified

### 1. Knight Facing Wrong Direction
**Problem**: Knights are facing along the board (toward the h-file or a-file) instead of across the board (toward the opponent).

**Root Cause**: The knight GLB model is oriented such that the horse faces along the X-axis by default. The current `piece_rotation()` function only applies a 180° Y-rotation for black pieces, which keeps them facing along X.

**Current Behavior**:
- White knights: Face +X (toward h-file)
- Black knights: Face -X (toward a-file)

**Expected Behavior**:
- White knights: Face +Z (toward rank 8, black's side)
- Black knights: Face -Z (toward rank 1, white's side)

**Fix**: Add a 90° Y-rotation (+Z for white, -Z for black) specifically for knights.

### 2. Pieces Spawning Below Board
**Problem**: Pieces appear below the board surface during certain operations.

**Current System**:
```rust
// Board squares positioned at y=0
let world_pos = Vec3::new(file as f32, 0., rank as f32);

// Piece Y-offset
const PIECE_Y_OFFSET: f32 = -0.5;

// Per-piece offsets (all have y=-0.5)
const KING_OFFSET: Vec3 = Vec3::new(-0.2, -0.5, -1.9);
const QUEEN_OFFSET: Vec3 = Vec3::new(-0.2, -0.5, -0.95);
// ... etc
```

**Issue**: The Y-offset of -0.5 assumes a specific model height. If the model's origin is at its geometric center and the height is ~5.5 units, with scale 0.18:
- Scaled height = 5.5 * 0.18 = 0.99
- Half height = 0.495 ≈ 0.5
- So Y-offset = -0.5 places base at y=0 ✓

However, the varying Z-offsets (-1.9 to 2.6) suggest the GLB models have different center positions, which may be causing visual misalignment.

## Coordinate System Reference

### Board Coordinates
- X: File (0-7, a-h) → World X (0.0 to 7.0)
- Z: Rank (0-7, 1-8) → World Z (0.0 to 7.0)
- Y: Board surface at 0.0

### Piece Placement
- White pieces: Ranks 0-1 (world Z = 0.0 to 1.0)
- Black pieces: Ranks 6-7 (world Z = 6.0 to 7.0)

### Rotation Reference
- Y-axis rotation: 0° = facing +Z, 90° = facing +X, 180° = facing -Z, 270° = facing -X

## Implementation Plan

### Phase 1: Fix Knight Rotation
1. Create a specialized `knight_rotation(color: PieceColor) -> Quat` function
2. Apply 90° additional rotation to align knight with Z-axis
3. Test knight orientation for both colors

### Phase 2: Standardize Positioning
1. Document the coordinate system clearly in code comments
2. Create a unified `calculate_piece_transform()` utility
3. Ensure all spawn locations use consistent coordinate calculations

### Phase 3: Verify Y-Positioning
1. Confirm Y-offset calculation matches actual model bounds
2. Add debug visualization option for piece bounding boxes
3. Test in both Standard and TempleOS view modes

## Code Changes Required

### File: `src/rendering/pieces/pieces.rs`

1. Add knight-specific rotation function:
```rust
fn knight_rotation(color: PieceColor) -> Quat {
    match color {
        // White: Face +Z (toward black's side), need 90° rotation from default X-facing
        PieceColor::White => Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2),
        // Black: Face -Z (toward white's side), need 90° + 180° = 270° (-90°)
        PieceColor::Black => Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
    }
}
```

2. Update `spawn_knight` to use the new rotation function instead of the generic `piece_rotation()`.

3. Document the coordinate system at module level:
```rust
//! Coordinate System:
//! - World X: File (a-h) mapped to 0.0-7.0
//! - World Z: Rank (1-8) mapped to 0.0-7.0  
//! - World Y: Vertical, board surface at Y=0.0
//! - Rotation Y=0: Facing +Z (toward rank 8 from white's perspective)
```

## Testing Checklist

- [ ] White knights at b1, g1 face toward black's side (+Z direction)
- [ ] Black knights at b8, g8 face toward white's side (-Z direction)
- [ ] All pieces sit properly on board surface (Y=0)
- [ ] Pieces appear correctly in Standard view mode
- [ ] Pieces appear correctly in TempleOS view mode (if applicable)
- [ ] No pieces appear below or floating above board
- [ ] Piece picking/selection works correctly after fixes
