# XFChess

XFChess is a modern, 3D chess game implementation built with Rust and the Bevy game engine. It features a comprehensive state management system, networked multiplayer, and high-quality rendering.

## Features

### Core Gameplay
- **Chess Engine**: Custom implementation of chess rules with Minimax AI and alpha-beta pruning (approx. 1800-2000 ELO).
- **Game Modes**: 
  - Human vs Human (Local)
  - Human vs AI
  - TempleOS View Mode (Board visualization)
- **Move Validation**: Full validation of legal moves, including castling, en passant, and check/checkmate detection.

### Networking & Multiplayer
- **Multiplayer Support**: Networked play powered by `lightyear`.
- **Lobby System**: Create and join game lobbies.
- **Chat**: In-game chat functionality.

### 3D Rendering & Visuals
- **High-Quality Graphics**: 3D chess pieces and board with dynamic lighting.
- **Visual Effects**: 
  - Move highlights and valid move indicators.
  - Smooth piece animations for moves and captures.
  - Atmospheric effects (fog/lighting).
- **Camera Controls**: Orbit camera with zoom and rotation support.

### User Interface
- **Modern UI**: Built with `bevy_egui` for a responsive and clean interface.
- **Menus**: 
  - Main Menu with 3D background scene.
  - Settings Menu (Graphics, Audio, Gameplay).
  - Piece Viewer (Inspect 3D models).
  - Multiplayer Lobby.
- **HUD**: In-game status display (Turn indicator, Timer, FPS).

### Technical Features
- **Cross-Platform**: Supports Desktop (Windows/Linux/Mac) and Web (WASM).
- **State Management**: Robust architecture using Bevy States (`GameState`, `MainMenu`, `InGame`, etc.).
- **Asset Management**: Centralized asset loading with progress bars.
- **Performance**: Optimized ECS architecture with explicit system ordering.
- **Debugging**: Integrated Inspector (WorldInspectorPlugin) and file-based logging.

## Controls

- **Mouse Left Click**: Select piece / Move piece.
- **Mouse Right Click / Drag**: Rotate camera.
- **Mouse Scroll**: Zoom in/out.
- **ESC**: Pause game / Open menu.
- **F11**: Toggle Fullscreen.
- **F1**: Toggle Inspector (Debug).
- **F12**: Toggle Debug Info.

## Build & Run

### Desktop
```bash
cargo run --release
```

### Web (WASM)
```bash
cargo run --target wasm32-unknown-unknown
```
(Requires `wasm-server-runner` or similar setup)

## License

This project is licensed under the MIT License.
