# Engine Module

## Purpose

The Engine module provides the chess engine functionality for XFChess, handling board state management, move validation, and position evaluation. It wraps the `shakmaty` chess library to provide a Bevy-compatible resource for game logic.

## Impact on Game

This module ensures:
- **Rule Enforcement**: Validates all moves according to standard chess rules
- **Position Tracking**: Maintains authoritative game state in FEN notation
- **Legal Move Generation**: Calculates all valid moves for any piece
- **Game Status Detection**: Identifies check, checkmate, stalemate, and draw conditions
- **AI Integration**: Provides position data for Stockfish AI analysis

## Architecture/Key Components

### Core Structure

| Component | Purpose |
|-----------|---------|
| [`ChessEngine`](board_state.rs:25) | Bevy Resource holding board position and game state |
| [`STARTING_FEN`](board_state.rs:18) | Standard chess starting position notation |

### Key Features

- **FEN-based Position**: Uses Forsyth-Edwards Notation for universal chess position representation
- **Move Validation**: Leverages `shakmaty` for legal move generation
- **Coordinate Conversion**: Translates between ECS coordinates (0-7) and UCI notation (a1-h8)
- **Game State Tracking**: Halfmove clock, fullmove counter, castling rights, en passant

### Coordinate Systems

| System | Format | Example |
|--------|--------|---------|
| ECS | `(x, y)` where 0,0 = a1 | `(4, 4)` = e5 |
| UCI | Algebraic notation | `"e4"`, `"Nf3"` |
| FEN | Full board description | `rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR` |

## Usage

### Initializing the Engine

```rust
fn setup_engine(mut commands: Commands) {
    commands.insert_resource(ChessEngine::default());
}
```

### Validating Moves

```rust
fn make_move(
    mut engine: ResMut<ChessEngine>,
    from: (u8, u8),
    to: (u8, u8),
) -> bool {
    let from_uci = ChessEngine::coords_to_uci(from.0, from.1);
    let to_uci = ChessEngine::coords_to_uci(to.0, to.1);
    let move_str = format!("{}{}", from_uci, to_uci);
    
    engine.is_move_legal(&move_str)
}
```

### Getting Legal Moves

```rust
fn show_legal_moves(
    engine: Res<ChessEngine>,
    square: (u8, u8),
) -> Vec<String> {
    let sq = ChessEngine::coords_to_uci(square.0, square.1);
    engine.get_legal_moves(&sq)
}
```

### Converting Coordinates

```rust
// ECS (4, 4) → UCI "e5"
let uci = ChessEngine::coords_to_uci(4, 4);

// UCI "e5" → ECS (4, 4)
let coords = ChessEngine::uci_to_coords("e5");
```

## Dependencies

- [`shakmaty`](https://docs.rs/shakmaty) - Rust chess library for move validation
- `bevy` - ECS integration

## Related Modules

- [`game`](../game/README.md) - Uses engine for move validation
- [`singleplayer`](../singleplayer/README.md) - AI uses engine positions
- [`rendering`](../rendering/README.md) - Visualizes engine state

## External References

- [FEN Notation](https://en.wikipedia.org/wiki/Forsyth%E2%80%93Edwards_Notation) - Wikipedia
- [UCI Protocol](https://en.wikipedia.org/wiki/Universal_Chess_Interface) - Chess engine communication
- [Shakmaty Docs](https://docs.rs/shakmaty) - Library documentation
