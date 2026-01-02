# XFChess

A modern 3D chess game built with Bevy 0.17, featuring beautiful graphics, AI opponent, and an innovative TempleOS-inspired view mode.

## Features

- **3D Chess Gameplay** - Beautifully rendered 3D chess pieces and board
- **AI Opponent** - Play against an AI with adjustable difficulty (Easy, Medium, Hard)
- **View Modes**
  - Standard: Classic 3D chess with piece meshes
  - TempleOS: Retro text-based view with ASCII-style board
- **Move Validation** - Full chess rules implementation with legal move checking
- **Move History** - Track all moves made during the game
- **Sound Effects** - Audio feedback for moves and captures
- **Settings Persistence** - Save your preferences (volume, graphics quality, themes)
- **Web Support** - WASM build for browser play (experimental)

## Quick Start

### Prerequisites

- Rust 1.70+ (install from [rustup.rs](https://rustup.rs/))
- Git

### Running the Game

```powershell
# Clone the repository
git clone https://github.com/yourusername/XFChess.git
cd XFChess

# Run in development mode
cargo run
```

Or use the provided scripts:
- Windows: `.\run.bat` or `.\run.ps1`

### Building for Release

```powershell
cargo build --release
```

The optimized binary will be in `target/release/xfchess.exe`

## Project Structure

```
XFChess/
├── src/                    # Main application code
│   ├── game/              # Game logic, AI, and components
│   ├── rendering/         # 3D rendering, pieces, board
│   ├── states/            # Game states (menu, in-game, settings)
│   └── ui/                # UI components and themes
├── crates/
│   └── chess_engine/      # Core chess engine (move generation, validation)
├── assets/               # 3D models, textures, sounds
│   ├── models/           # GLTF piece models
│   ├── sounds/           # Audio files
│   └── textures/         # Board and UI textures
├── web/                  # WASM web build (experimental)
└── .cargo/              # Stack size config for GLTF parsing
```

## Controls

- **Left Click** - Select piece / Move piece
- **Drag & Drop** - Drag pieces to move them
- **ESC** - Open/close menu
- **Camera** - Orbital camera with smooth animations

## Technical Details

### Built With

- **Bevy 0.17** - Game engine
- **Chess Engine** - Custom move generation and validation
- **bevy_egui** - UI framework
- **bevy-inspector-egui** - Debug inspector

### Architecture Highlights

- **ECS-based** - Entity Component System for game state management
- **Data-driven** - Piece spawning uses const arrays instead of hardcoded entities
- **Observable Components** - Piece interactions use Bevy's observer pattern
- **GLTF Asset Loading** - Efficient mesh loading with precomputed tables

### Performance Optimizations

- 8MB stack size for parallel GLTF parsing
- Mesh instancing for identical pieces
- Optimized move generation with bitboards
- Asset preloading system

## Development

### Debug Mode

The game includes a debug inspector accessible in development builds:
- View entity hierarchy
- Inspect component values
- Monitor resource state

### Testing

```powershell
# Run all tests
cargo test

# Run chess engine tests
cargo test --package chess_engine

# Run specific test
cargo test test_pawn_movement
```

## Known Issues

- Web build (WASM) is experimental and may have performance issues
- Some advanced chess rules (en passant, castling) are work-in-progress

## License

[Add your license here]

## Credits

- Chess piece models: [chess_kit](https://github.com/bevy-chessengine/bevy-chess)
- Bevy Engine: [bevyengine.org](https://bevyengine.org/)

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
