# Input Module

## Purpose

The Input module handles all user input for XFChess, including mouse picking, drag-and-drop interactions, and pointer events. It provides the bridge between player actions and game responses using Bevy's input systems.

## Impact on Game

This module enables:
- **Piece Selection**: Clicking to select chess pieces
- **Drag and Drop**: Moving pieces by dragging them to target squares
- **Precise Picking**: Raycasting from screen coordinates to 3D board positions
- **Multi-button Support**: Left click (select/move), Right click (cancel/context)
- **Responsive Controls**: Low-latency input handling for smooth gameplay

## Architecture/Key Components

### Core Systems

| Component | Purpose |
|-----------|---------|
| [`InputPlugin`](mod.rs:8) | Bevy plugin registering all input systems |
| [`pointer.rs`](pointer.rs) | Pointer/picking implementation for 3D interaction |
| [`PointerInput`](pointer.rs) | Resource tracking pointer state and interactions |

### Input Pipeline

```
Mouse/Pointer Event → Screen Coordinates → Raycast → 3D Intersection → Game Response
```

### Key Features

- **Raycasting**: Casts rays from camera through mouse position to intersect with board squares
- **Entity Picking**: Identifies which 3D entity (piece) is under the cursor
- **Drag Detection**: Tracks mouse movement between press and release
- **Coordinate Mapping**: Converts screen input to board coordinates (0-7, 0-7)

## Usage

### Handling Piece Selection

```rust
fn piece_selection(
    pointer: Res<PointerInput>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    pieces: Query<(Entity, &Piece, &Transform)>,
) {
    if mouse_button.just_pressed(MouseButton::Left) {
        if let Some((entity, piece, _)) = pointer.hovered_entity
            .and_then(|e| pieces.get(e).ok()) 
        {
            // Select the piece
        }
    }
}
```

### Handling Drag and Drop

```rust
fn drag_and_drop(
    mut pointer: ResMut<PointerInput>,
    mouse_button: Res<ButtonInput<MouseButton>>,
) {
    if mouse_button.pressed(MouseButton::Left) {
        pointer.is_dragging = true;
        pointer.drag_current = pointer.world_position;
    }
    
    if mouse_button.just_released(MouseButton::Left) {
        if pointer.is_dragging {
            // Process the move
            let from = pointer.drag_start;
            let to = pointer.world_position;
            // Convert to board coordinates and validate move
        }
        pointer.is_dragging = false;
    }
}
```

### Converting Screen to Board Coordinates

```rust
fn screen_to_board(
    windows: Query<&Window>,
    camera: Query<(&Camera, &GlobalTransform)>,
) -> Option<(u8, u8)> {
    let window = windows.single();
    let (camera, camera_transform) = camera.single();
    
    if let Some(cursor) = window.cursor_position() {
        // Raycast from camera through cursor position
        // Intersect with board plane
        // Convert intersection to board coordinates (0-7, 0-7)
    }
    None
}
```

## Dependencies

- [`bevy::input`](https://docs.rs/bevy/latest/bevy/input/index.html) - Core input handling
- [`bevy::window`](https://docs.rs/bevy/latest/bevy/window/index.html) - Window and cursor access
- [`bevy::render`](https://docs.rs/bevy/latest/bevy/render/index.html) - Camera raycasting

## Related Modules

- [`game`](../game/README.md) - Processes input into game actions
- [`rendering`](../rendering/README.md) - Provides camera and 3D context for picking
- [`engine`](../engine/README.md) - Validates moves from input

## Edge Cases

- **Multiple Cameras**: Input system handles active camera selection
- **Off-board Clicks**: Clicks outside the board are ignored or cancel selection
- **Simultaneous Inputs**: System prioritizes left-click for gameplay
