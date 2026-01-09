# Chess Engine Testing Guide

The `chess_engine` crate is the core AI and rules engine. It requires thorough testing.

## Module Structure

```
crates/chess_engine/src/
├── lib.rs          # Public API exports
├── api.rs          # new_game, reply, do_move, is_legal_move
├── bitset.rs       # 64-bit bitboard operations
├── board.rs        # Board state and queries
├── constants.rs    # Piece values, flags
├── evaluation.rs   # Position scoring
├── move_gen.rs     # Legal move generation
├── search.rs       # Minimax with alpha-beta
├── types.rs        # Game, Move, Board structs
└── utils.rs        # Helper functions
```

## API Tests

### Testing Game Initialization

```rust
// crates/chess_engine/src/api.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_game_creates_valid_board() {
        let game = new_game();
        
        // Check starting position
        assert_eq!(game.board[0], ROOK);    // a1 = white rook
        assert_eq!(game.board[4], KING);    // e1 = white king
        assert_eq!(game.board[60], -KING);  // e8 = black king
        
        // Check game state
        assert_eq!(get_game_state(&game), STATE_PLAYING);
    }

    #[test]
    fn test_legal_move_e2_e4() {
        let game = new_game();
        let from = square_to_index(4, 1); // e2
        let to = square_to_index(4, 3);   // e4
        
        assert!(is_legal_move(&game, from, to));
    }

    #[test]
    fn test_illegal_move_e1_e3() {
        let game = new_game();
        let from = square_to_index(4, 0); // e1 (king)
        let to = square_to_index(4, 2);   // e3
        
        assert!(!is_legal_move(&game, from, to), "King cannot move two squares");
    }
}
```

### Testing Move Execution

```rust
#[test]
fn test_do_move_updates_board() {
    let mut game = new_game();
    let from = square_to_index(4, 1); // e2
    let to = square_to_index(4, 3);   // e4
    
    do_move(&mut game, from, to, true);
    
    assert_eq!(game.board[from as usize], 0, "Source square should be empty");
    assert_eq!(game.board[to as usize], PAWN, "Destination should have pawn");
}

#[test]
fn test_do_move_alternates_turn() {
    let mut game = new_game();
    assert_eq!(game.to_move, COLOR_WHITE);
    
    do_move(&mut game, e2, e4, true);
    assert_eq!(game.to_move, COLOR_BLACK);
    
    do_move(&mut game, e7, e5, true);
    assert_eq!(game.to_move, COLOR_WHITE);
}
```

## Move Generation Tests

### Testing Pawn Moves

```rust
// crates/chess_engine/src/move_gen.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pawn_starting_moves() {
        let game = new_game();
        let e2 = square_to_index(4, 1);
        let moves = generate_pseudo_legal_moves(&game, e2);
        
        // Pawn on e2 can move to e3 or e4
        assert!(moves.contains(&square_to_index(4, 2))); // e3
        assert!(moves.contains(&square_to_index(4, 3))); // e4
        assert_eq!(moves.len(), 2);
    }

    #[test]
    fn test_pawn_blocked_cannot_move() {
        let mut game = new_game();
        // Place a piece on e3
        game.board[square_to_index(4, 2) as usize] = KNIGHT;
        
        let moves = generate_pseudo_legal_moves(&game, square_to_index(4, 1));
        assert!(moves.is_empty(), "Blocked pawn has no moves");
    }

    #[test]
    fn test_pawn_capture() {
        let mut game = new_game();
        // Place black pawn on d3
        game.board[square_to_index(3, 2) as usize] = -PAWN;
        
        let moves = generate_pseudo_legal_moves(&game, square_to_index(4, 1));
        assert!(moves.contains(&square_to_index(3, 2)), "Can capture on d3");
    }
}
```

### Testing Knight Moves

```rust
#[test]
fn test_knight_moves_from_b1() {
    let game = new_game();
    let b1 = square_to_index(1, 0);
    let moves = generate_pseudo_legal_moves(&game, b1);
    
    // Knight on b1 can move to a3 or c3
    assert!(moves.contains(&square_to_index(0, 2))); // a3
    assert!(moves.contains(&square_to_index(2, 2))); // c3
    assert_eq!(moves.len(), 2);
}
```

## Evaluation Tests

```rust
// crates/chess_engine/src/evaluation.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_starting_position_equal() {
        let game = new_game();
        let score = evaluate(&game);
        assert_eq!(score, 0, "Starting position is balanced");
    }

    #[test]
    fn test_white_up_queen_is_winning() {
        let mut game = new_game();
        game.board[59] = 0; // Remove black queen from d8
        
        let score = evaluate(&game);
        assert!(score > 800, "White should be +9 pawns (900 centipawns)");
    }

    #[test]
    fn test_checkmate_is_max_score() {
        let game = setup_checkmate_position();
        let score = evaluate(&game);
        assert!(score.abs() > SURE_CHECKMATE);
    }
}
```

## Search Tests

```rust
// crates/chess_engine/src/search.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_finds_mate_in_one() {
        // Setup position where Qh7# is mate
        let mut game = setup_mate_in_one();
        game.secs_per_move = 1.0;
        
        let best_move = reply(&mut game);
        
        assert_eq!(best_move.dst, square_to_index(7, 6)); // h7
        assert_eq!(best_move.state, STATE_CHECKMATE);
    }

    #[test]
    fn test_ai_avoids_losing_queen() {
        let mut game = setup_queen_hanging();
        game.secs_per_move = 1.0;
        
        let best_move = reply(&mut game);
        
        // AI should move the queen
        let moved_piece = game.board[best_move.src as usize];
        assert_eq!(moved_piece.abs(), QUEEN);
    }
}
```

## Test Helpers

```rust
// crates/chess_engine/src/test_utils.rs (new file)
#![cfg(test)]

use crate::*;

/// Create a game from FEN string (Forsyth-Edwards Notation)
pub fn from_fen(fen: &str) -> Game {
    // Parse FEN and create game
    unimplemented!()
}

/// Square name to index: "e4" -> 28
pub fn sq(name: &str) -> i8 {
    let chars: Vec<char> = name.chars().collect();
    let file = (chars[0] as u8 - b'a') as i8;
    let rank = (chars[1] as u8 - b'1') as i8;
    rank * 8 + file
}

/// Setup scholars mate position
pub fn setup_mate_in_one() -> Game {
    let mut game = new_game();
    // Qh5, Bc4 vs e5, Nc6
    // ... setup moves
    game
}
```

## Running Engine Tests

```bash
# Run all engine tests
cargo test -p chess_engine

# Run with output
cargo test -p chess_engine -- --nocapture

# Run specific test
cargo test -p chess_engine test_ai_finds_mate

# Run slow tests (AI search)
cargo test -p chess_engine --release -- --ignored
```
