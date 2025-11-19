---
description: Repository Information Overview
alwaysApply: true
---

# XFChess Repository Overview

## Repository Summary

XFChess is a **Bevy-powered chess game engine** with visual debugging capabilities. Built as a multi-project Rust workspace, it provides a complete chess implementation with interactive UI, real-time component debugging via egui inspector, and both desktop and web deployment targets. The project includes optimized development profiles with level-3 dependency compilation for faster iteration.

## Repository Structure

The project is organized as a Cargo workspace with three main components:

### Main Repository Components
- **Desktop Application** (`src/`): Bevy ECS-based native chess game with rendering, input handling, game states, and AI systems
- **Chess Engine Library** (`crates/chess_engine/`): Shared chess logic including move generation, evaluation, alpha-beta search, and bitboard operations
- **Web Application** (`web/`): Leptos framework with WASM bindings for browser-based chess gameplay
- **Assets** (`assets/`): Game sounds and 3D models for board and pieces
- **Documentation** (`docs/`): Graphics modeling guides and quick-start references

## Projects

### Desktop Game (xfchess)
**Configuration File**: `Cargo.toml` (root)

#### Language & Runtime
**Language**: Rust
**Edition**: 2021
**Build System**: Cargo workspace
**Package Manager**: Cargo

#### Main Dependencies
- **Bevy**: 0.17.2 (game engine, ECS framework, rendering)
- **Bevy Egui**: 0.37.1 (UI widget system)
- **Bevy Inspector Egui**: 0.34.0 (real-time component debugging)
- **Futures Lite**: 2.5.0 (async runtime)
- **Rand**: 0.8 (random number generation)
- **Serde/Serde JSON**: 1.0 (serialization)
- **Thiserror**: 1.0 (error handling)
- **Chess Engine**: Local path dependency

#### Source Structure
- `main.rs`: Application entry point and Bevy app setup
- `core/`: System initialization, error handling, window config, settings persistence
- `game/`: Game logic components, resources, systems, AI, game types
- `rendering/`: Board rendering, camera management, piece visuals, effects, graphics quality
- `input/`: Mouse pointer event handling and interaction
- `ui/`: Game UI, inspector panel, styles
- `states/`: Game state machines (splash, menu, game, pause, game over, settings, piece viewer)
- `assets/`: Asset management and loading
- `audio/`: Audio system integration

#### Build & Installation
```bash
cargo build --release
cargo run --release
```

**Windows**: Use `run.bat` or `run.ps1` scripts which set `RUST_MIN_STACK=134217728` environment variable.

#### Testing
**Framework**: Rust integrated testing
**Test Location**: `tests/core_tests.rs`
**Configuration**: Integration tests for core systems
**Run Command**:
```bash
cargo test
```

---

### Chess Engine (chess_engine)
**Configuration File**: `crates/chess_engine/Cargo.toml`

#### Language & Runtime
**Language**: Rust
**Edition**: 2021
**Library Type**: cdylib, rlib (native and WASM compatible)

#### Dependencies
**Main**: Thiserror 1.0 (error types)
**Features**: `salewskiChessDebug` debug feature available

#### Module Structure
- `api/`: Public game API (game state, moves, state management)
- `move_gen/`: Move generation (piece-specific implementations: pawn, knight, bishop, rook, queen, king, attack tables)
- `search/`: Alpha-beta pruning, quiescence search, iterative deepening, move ordering
- `evaluation/`: Material scoring, position evaluation, piece-square tables
- `bitset.rs`: Bitboard operations
- `constants.rs`: Chess constants and piece definitions
- `types.rs`: Core chess types and enums
- `lib.rs`: Library root with public API exports

---

### Web Application (xfchess-web)
**Configuration File**: `web/Cargo.toml`

#### Language & Runtime
**Language**: Rust (compiled to WebAssembly)
**Edition**: 2021
**Target**: wasm32-unknown-unknown
**WASM Crate Types**: cdylib, rlib

#### Main Dependencies
- **Leptos**: 0.7 (client-side rendering framework)
- **Bevy**: 0.17.2 (game engine with web features)
- **Wasm-Bindgen**: 0.2.105 (JavaScript interop)
- **Web-sys**: 0.3 (browser APIs: Document, Element, Canvas, Window)
- **Console Log**: 1.0 (browser console logging)
- **Log**: 0.4 (logging facade)

#### Build & Deployment
```bash
# Unix/macOS
./build-wasm.sh

# Windows
build-wasm.bat
```

**Build Process**: Compiles to WASM, generates JavaScript bindings, outputs to `web/pkg/`
**Serving Options**:
```bash
cd web && trunk serve        # Using Trunk build tool
cd web && python3 -m http.server  # Simple HTTP server
```

#### Source Structure
- `lib.rs`: Library root (WASM entry point)
- `app.rs`: Leptos application component
- `bevy_wasm.rs`: Bevy initialization for WASM
- `main.rs`: Client bootstrap

#### Configuration
**Optimization Profile** (Release):
- opt-level: 'z' (minimal size)
- codegen-units: 1 (maximum optimization)
- lto: true (full link-time optimization)

#### Testing
**Framework**: Wasm-bindgen-test
**Dev Dependencies**: wasm-bindgen-test 0.3

---

## Build Optimization

All projects use aggressive optimization settings:
- **Release Profile**: opt-level 3, thin LTO, stripped symbols, single codegen unit
- **Development Profile**: Dependencies compiled with opt-level 3 for faster iteration
- **WASM Profile**: Minimal binary size with full optimization for browser deployment

## Development Setup

**Requirements**: Rust 1.70+ (MSRV from Cargo.lock)

**Initial Setup**:
```bash
rustup target add wasm32-unknown-unknown  # For WASM builds
cargo install wasm-bindgen-cli             # For WASM binding generation
```

**Development Commands**:
```bash
cargo build          # Debug build
cargo build --release  # Optimized build
cargo test          # Run all tests
cargo run --release  # Run desktop game
```

**Stack Configuration**: WASM tasks use 128MB stack (configured in `.cargo/config.toml`)
