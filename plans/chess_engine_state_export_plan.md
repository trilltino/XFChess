# ChessEngine State Export Implementation Plan

## Problem
The `ChessEngineExt` trait in `board_state.rs` has placeholder implementations:
- `to_fen_string()` - hardcoded to starting position
- `get_move_counter()` - returns 0
- `get_current_turn()` - returns White

This prevents the P2P board state sync from working.

## Solution Overview
Add methods to `ChessEngine` to export its internal state in FEN format and track game progression.

## Implementation Steps

### 1. Add State Tracking to ChessEngine

**File:** `src/engine/board_state.rs`

Add these fields to `ChessEngine` struct:
```rust
pub struct ChessEngine {
    // ... existing fields ...
    
    /// Full move counter (increments after Black's move)
    pub move_counter: u32,
    
    /// Halfmove clock for 50-move rule
    pub halfmove_clock: u32,
    
    /// Current turn (White or Black)
    pub current_turn: PieceColor,
    
    /// Move history for undo/debugging
    pub move_history: Vec<MoveRecord>,
    
    /// Castling rights (KQkq)
    pub castling_rights: String,
    
    /// En passant target square (if any)
    pub en_passant: Option<String>,
}
```

### 2. Initialize State in Constructor

Update `ChessEngine::new()` or initialization:
```rust
impl ChessEngine {
    pub fn new() -> Self {
        Self {
            // ... existing fields ...
            move_counter: 1,  // Starts at 1
            halfmove_clock: 0,
            current_turn: PieceColor::White,
            move_history: Vec::new(),
            castling_rights: "KQkq".to_string(),
            en_passant: None,
        }
    }
}
```

### 3. Implement FEN Export

Add method to export board as FEN string:
```rust
impl ChessEngine {
    /// Export current board state as FEN string
    /// Format: rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1
    pub fn to_fen(&self) -> String {
        let piece_placement = self.board_to_fen_piece_placement();
        let active_color = match self.current_turn {
            PieceColor::White => "w",
            PieceColor::Black => "b",
        };
        let castling = if self.castling_rights.is_empty() {
            "-".to_string()
        } else {
            self.castling_rights.clone()
        };
        let en_passant = self.en_passant.as_deref().unwrap_or("-");
        
        format!(
            "{} {} {} {} {} {}",
            piece_placement,
            active_color,
            castling,
            en_passant,
            self.halfmove_clock,
            self.move_counter
        )
    }
    
    /// Convert internal board representation to FEN piece placement
    fn board_to_fen_piece_placement(&self) -> String {
        // Iterate ranks 8 to 1
        // For each rank, iterate files a to h
        // Build string with piece letters and empty square counts
        // Use uppercase for White, lowercase for Black
        // Example: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR"
        
        // This depends on how ChessEngine stores board state
        // Could be bitboards, 8x8 array, piece list, etc.
        todo!("Implement based on internal board representation")
    }
}
```

### 4. Implement State Update on Move

Update state after each move:
```rust
impl ChessEngine {
    /// Apply a move and update all state tracking
    pub fn apply_move(&mut self, from: (u8, u8), to: (u8, u8), piece: PieceType) -> Result<(), EngineError> {
        // ... existing move validation and application ...
        
        // Update turn
        self.current_turn = match self.current_turn {
            PieceColor::White => PieceColor::Black,
            PieceColor::Black => PieceColor::White,
        };
        
        // Update move counter after Black's move
        if self.current_turn == PieceColor::White {
            self.move_counter += 1;
        }
        
        // Update halfmove clock
        // Reset on pawn move or capture, increment otherwise
        if piece == PieceType::Pawn || is_capture {
            self.halfmove_clock = 0;
        } else {
            self.halfmove_clock += 1;
        }
        
        // Update castling rights if king or rook moved
        self.update_castling_rights(from, piece);
        
        // Update en passant target
        self.update_en_passant(from, to, piece);
        
        // Record move in history
        self.move_history.push(MoveRecord {
            from,
            to,
            piece,
            // ... other fields ...
        });
        
        Ok(())
    }
}
```

### 5. Implement FEN Import

Add method to import board from FEN:
```rust
impl ChessEngine {
    /// Import board state from FEN string
    pub fn from_fen(fen: &str) -> Result<Self, EngineError> {
        let parts: Vec<&str> = fen.split_whitespace().collect();
        if parts.len() != 6 {
            return Err(EngineError::InvalidFen);
        }
        
        let mut engine = Self::new();
        
        // Parse piece placement
        engine.fen_piece_placement_to_board(parts[0])?;
        
        // Parse active color
        engine.current_turn = match parts[1] {
            "w" => PieceColor::White,
            "b" => PieceColor::Black,
            _ => return Err(EngineError::InvalidFen),
        };
        
        // Parse castling rights
        engine.castling_rights = if parts[2] == "-" {
            String::new()
        } else {
            parts[2].to_string()
        };
        
        // Parse en passant
        engine.en_passant = if parts[3] == "-" {
            None
        } else {
            Some(parts[3].to_string())
        };
        
        // Parse halfmove clock
        engine.halfmove_clock = parts[4].parse()?;
        
        // Parse fullmove number
        engine.move_counter = parts[5].parse()?;
        
        Ok(engine)
    }
}
```

### 6. Update ChessEngineExt Trait

Replace placeholder implementations with actual calls:
```rust
impl ChessEngineExt for ChessEngine {
    fn to_fen_string(&self) -> String {
        self.to_fen()
    }

    fn get_move_counter(&self) -> u32 {
        self.move_counter
    }

    fn get_current_turn(&self) -> PieceColor {
        self.current_turn
    }
}
```

### 7. Integration Points

**In `shared.rs` - after move execution:**
```rust
// Update engine state
engine.apply_move(from, to, piece_type)?;

// Now FEN export will reflect actual state
let fen = engine.to_fen_string();
```

**In `board_state.rs` - state broadcast:**
```rust
// This will now work correctly
let serialized = sync.serialize_state(engine, captured_pieces, Some(board_move));
```

## Testing Strategy

1. **Unit Tests for FEN Export/Import:**
```rust
#[test]
fn test_fen_roundtrip() {
    let engine = ChessEngine::new();
    let fen = engine.to_fen();
    let engine2 = ChessEngine::from_fen(&fen).unwrap();
    assert_eq!(engine.to_fen(), engine2.to_fen());
}

#[test]
fn test_fen_after_move() {
    let mut engine = ChessEngine::new();
    engine.apply_move((4, 1), (4, 3), PieceType::Pawn).unwrap();
    let fen = engine.to_fen();
    assert!(fen.contains("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR"));
}
```

2. **Integration Test - P2P Sync:**
- Alice makes move
- State serialized and sent to Bob
- Bob applies state
- Both boards match

## Files to Modify

1. `src/engine/board_state.rs` - Add FEN export/import and state tracking
2. `src/game/sync/board_state.rs` - Update ChessEngineExt to use real methods
3. `src/game/systems/shared.rs` - Call engine.apply_move() after execution

## Estimated Effort

- FEN export: 2-3 hours (depends on internal board representation)
- FEN import: 1-2 hours
- State tracking: 1 hour
- Testing: 1-2 hours
- **Total: 5-8 hours**

## Alternative Quick Fix

If full FEN is too complex, implement a simpler state format:
```rust
/// Simple state: list of all pieces with positions
pub fn to_simple_state(&self) -> String {
    // Format: "Ka1,Qd1,Ra1...:ka8,qd8,ra8...:move_counter:turn"
    // White pieces : Black pieces : move count : turn
}
```

This is easier to implement but not standard FEN.
