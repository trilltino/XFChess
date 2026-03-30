# Rendering Module

## Purpose

The Rendering module handles all 3D visualization for XFChess, including the chess board, pieces, camera controls, lighting effects, and visual feedback. It transforms the abstract game state into a visually appealing 3D chess experience.

## Impact on Game

This module delivers:
- **3D Chess Board**: Visual representation of the chess board with customizable themes
- **Piece Models**: High-quality 3D chess piece rendering with materials
- **Dynamic Camera**: Smooth camera controls with multiple viewing angles
- **Visual Effects**: Last move highlighting, move hints, and dynamic lighting
- **Theme Support**: Multiple board themes (Classic, TempleOS, etc.)
- **Performance**: Efficient rendering for smooth 60+ FPS gameplay

## Architecture/Key Components

### Submodules

| Module | Purpose |
|--------|---------|
| [`board/`](board/) | Chess board mesh, materials, and themes |
| [`pieces/`](pieces/) | 3D piece models, spawning, and positioning |
| [`camera/`](camera/) | Camera controls, movement, and perspectives |
| [`effects/`](effects/) | Visual effects (highlights, lighting, hints) |
| [`utils.rs`](utils.rs) | Rendering utilities and helpers |

### Board System

| Component | Function |
|-----------|----------|
| [`BoardPlugin`](board/mod.rs) | Manages board visualization |
| [`BoardTheme`](board/board_theme.rs) | Theme configuration and materials |
| [`TempleOSBoardPlugin`](board/templeos_ui.rs) | TempleOS-style retro theme |

### Piece System

| Component | Function |
|-----------|----------|
| [`PiecePlugin`](pieces/mod.rs) | Handles all piece rendering |
| [`Piece`](pieces/pieces.rs) | Component identifying piece type and color |
| [`PieceType`](pieces/pieces.rs) | Enum: King, Queen, Rook, Bishop, Knight, Pawn |
| [`PieceColor`](pieces/pieces.rs) | Enum: White, Black |

### Camera System

| Component | Function |
|-----------|----------|
| [`CameraPlugin`](camera/mod.rs) | Camera control and positioning |
| [`TempleOSCamera`](camera/camera_templeos.rs) | Special camera for TempleOS theme |

### Effects System

| Component | Function |
|-----------|----------|
| [`LastMoveHighlight`](effects/last_move.rs) | Highlights the most recent move |
| [`MoveHints`](effects/move_hints.rs) | Shows legal move indicators |
| [`DynamicLighting`](effects/dynamic_lighting.rs) | Adaptive lighting system |

## Usage

### Spawning a Piece

```rust
fn spawn_piece(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    position: (u8, u8),
) {
    let mesh = game_assets.piece_meshes.king.clone().unwrap();
    let material = materials.add(Color::WHITE);
    
    commands.spawn((
        PbrBundle {
            mesh,
            material,
            transform: Transform::from_xyz(
                position.0 as f32, 
                0.5, 
                position.1 as f32
            ),
            ..default()
        },
        Piece {
            piece_type: PieceType::King,
            color: PieceColor::White,
        },
    ));
}
```

### Changing Board Theme

```rust
fn set_theme(
    mut theme: ResMut<BoardTheme>,
) {
    theme.current = ThemeType::TempleOS;
    // Theme automatically updates materials
}
```

### Camera Control

```rust
fn focus_on_square(
    mut camera_query: Query<&mut Transform, With<GameCamera>>,
    target: (u8, u8),
) {
    if let Ok(mut transform) = camera_query.get_single_mut() {
        let target_pos = Vec3::new(
            target.0 as f32,
            0.0,
            target.1 as f32,
        );
        transform.translation = target_pos + Vec3::new(0.0, 10.0, 5.0);
        transform.look_at(target_pos, Vec3::Y);
    }
}
```

## Dependencies

- [`bevy::pbr`](https://docs.rs/bevy/latest/bevy/pbr/index.html) - 3D rendering and materials
- [`bevy::gltf`](https://docs.rs/bevy/latest/bevy/gltf/index.html) - GLTF model loading
- [`assets`](../assets/README.md) - Asset handles for models and materials

## Related Modules

- [`assets`](../assets/README.md) - Provides loaded 3D models
- [`game`](../game/README.md) - Drives piece positions
- [`input`](../input/README.md) - Raycasting for piece selection

## Performance Notes

- Pieces use instanced rendering where possible
- Board uses a single mesh with material switching for themes
- Shadows are optimized for the static board environment
