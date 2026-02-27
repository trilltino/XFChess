# Game Module

## Purpose
The Game module implements chess mechanics, move validation, board state management, and visual effects. It uses the `shakmaty` crate for rules enforcement and Bevy for presentation.

## Impact on Game
This is the **core gameplay layer**:
- **Move Validation:** Ensures only legal chess moves
- **Board State:** Tracks piece positions
- **Game Rules:** Enforces check, checkmate, stalemate
- **Visual Effects:** Move animations, highlights

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Game Module                             │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────┐    │
│  │    Board     │   │     Move     │   │    Visual    │    │
│  │    State     │   │  Validation  │   │    Effects   │    │
│  │  (shakmaty)  │   │  (shakmaty)  │   │    (Bevy)    │    │
│  └──────────────┘   └──────────────┘   └──────────────┘    │
│         │                  │                  │             │
│         └──────────────────┼──────────────────┘             │
│                            v                                │
│                   ┌──────────────┐                         │
│                   │ Game Events  │                         │
│                   │ (Bevy Events)│                         │
│                   └──────────────┘                         │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## Key Components

### Board State (`board.rs`)
Manages the chess board using shakmaty:
```rust
pub struct BoardState {
    pub position: Chess,
    pub turn: Color,
    pub move_stack: Vec<Move>,
}
```

### Move Validation
Uses shakmaty for legal move generation:
```rust
fn validate_move(board: &Board, from: Square, to: Square) -> bool {
    let legal_moves = board.legal_moves();
    legal_moves.iter().any(|m| {
        m.from() == Some(from) && m.to() == to
    })
}
```

### Visual Effects (`visual.rs`)
- Move highlights
- Last move indicator
- Check warning flash
- Capture animations

## Game Flow

### 1. Piece Selection
```
Player clicks piece
      │
      v
Check: Is it player's turn?
      │
      v
Check: Does piece belong to player?
      │
      v
Generate legal moves for piece
      │
      v
Highlight valid destination squares
```

### 2. Move Execution
```
Player clicks destination
      │
      v
Validate move is legal (shakmaty)
      │
      v
Check: Does move leave king in check?
      │
      v
Apply move to board state
      │
      v
Update visual representation
      │
      v
Check for game end (checkmate/stalemate)
      │
      v
If multiplayer: Send move to opponent
```

### 3. Opponent Move Reception
```
Receive move from network
      │
      v
Validate move on local board
      │
      v
Apply move
      │
      v
Update visuals
      │
      v
Check for game end
```

## Systems

### Input Handling
- `piece_selection_system` - Handle click on piece
- `move_execution_system` - Handle move completion
- `drag_and_drop_system` - Visual dragging

### State Management
- `board_setup_system` - Initialize board
- `turn_management_system` - Track whose turn
- `game_end_detection_system` - Checkmate/stalemate

### Visuals
- `move_highlight_system` - Show legal moves
- `last_move_indicator_system` - Highlight last move
- `capture_animation_system` - Piece capture FX

## Usage

### Make Move
```rust
fn make_move(
    mut board: ResMut<BoardState>,
    from: Square,
    to: Square,
) -> Result<(), MoveError> {
    let mv = board.find_legal_move(from, to)?;
    board.play(mv)?;
    Ok(())
}
```

### Get Legal Moves
```rust
fn get_legal_moves(
    board: &BoardState,
    square: Square,
) -> Vec<Square> {
    board.legal_moves_from(square)
}
```

## FEN Support

Convert to/from FEN notation:
```rust
// Export board state
let fen = board.to_fen();
// "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"

// Import board state
let board = BoardState::from_fen(fen)?;
```

## Testing

### Unit Tests
```bash
cargo test --package xfchess game::
```

### Integration Test
```rust
#[test]
fn test_checkmate_detection() {
    let board = BoardState::from_fen(
        "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 0 3"
    ).unwrap();
    
    assert!(board.is_checkmate());
}
```

## Dependencies

- `shakmaty` - Chess rules and move generation
- `bevy` - ECS and rendering

## Performance

- Move generation: ~100μs
- Validation: ~50μs
- FEN parsing: ~20μs
