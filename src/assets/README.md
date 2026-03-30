# Assets Module

## Purpose

The Assets module manages all game asset loading and tracking for XFChess. It provides a centralized resource management system for preloading and accessing 3D models, textures, sounds, and other game resources using Bevy's asset system.

## Impact on Game

This module ensures:
- **Visual Consistency**: All chess pieces, board textures, and UI elements are properly loaded before gameplay begins
- **Smooth User Experience**: Loading progress tracking allows for informative loading screens
- **Memory Management**: Asset handles are cached to prevent redundant loading and ensure assets stay in memory
- **Error Resilience**: Comprehensive error handling for missing or corrupted assets

## Architecture/Key Components

### Core Structures

| Component | Purpose |
|-----------|---------|
| [`GameAssets`](mod.rs:19) | Resource storing handles to all preloaded assets (GLTF models, meshes) |
| [`PieceMeshes`](mod.rs:41) | Container for individual chess piece mesh handles |
| [`LoadingProgress`](mod.rs:52) | Tracks loading progress (0.0 to 1.0) with completion status |

### Asset Types

- **Chess Pieces**: GLTF models loaded from `assets/models/chess_kit/pieces.glb`
- **Board Materials**: Textures and shaders for board themes
- **UI Assets**: Fonts, icons, and interface elements
- **Sound Effects**: Audio files for move sounds, captures, and UI feedback

### Loading Pipeline

```
Asset Request → Bevy AssetServer → Async Loading → Progress Tracking → Ready Signal
```

## Usage

### Accessing Assets

```rust
// In a system
fn spawn_piece(
    game_assets: Res<GameAssets>,
    mut commands: Commands,
) {
    if game_assets.loaded {
        let mesh = game_assets.piece_meshes.king.clone().unwrap();
        commands.spawn(PbrBundle {
            mesh,
            // ... other components
        });
    }
}
```

### Checking Loading Status

```rust
fn loading_screen(
    loading: Res<LoadingProgress>,
) {
    if loading.complete {
        println!("All assets loaded!");
    } else {
        println!("Loading: {}%", loading.percentage());
    }
}
```

## Dependencies

- [`bevy::asset`](https://docs.rs/bevy/latest/bevy/asset/index.html) - Core asset management
- [`bevy::gltf`](https://docs.rs/bevy/latest/bevy/gltf/index.html) - GLTF model loading

## Related Modules

- [`rendering`](../rendering/README.md) - Uses assets for 3D visualization
- [`game`](../game/README.md) - Uses piece meshes for spawning chess pieces
