# Component Tests

Unit tests for individual components of XFChess, focusing on isolated functionality testing.

## Overview

Component tests are unit tests that verify the correctness of individual components in isolation. These tests:
- Test single functions or methods
- Use mock data and fixtures
- Have fast execution times
- Are easy to debug when they fail

## Component Test Structure

```
tests/components/
├── mod.rs              # Module declaration
├── game_state_tests.rs # Game state management tests
└── piece_tests.rs      # Piece movement and validation tests
```

## Running Component Tests

```bash
# Run all component tests
cargo test components

# Run specific component test file
cargo test components::game_state_tests

# Run specific test
cargo test components::game_state_tests::test_initial_state
```

## Example: Game State Tests

This example shows component tests for game state management.

```rust
#[cfg(test)]
mod game_state_tests {
    use crate::game::state::{GameState, GameStatus, PlayerColor};
    
    #[test]
    fn test_initial_game_state() {
        let game_state = GameState::new();
        
        // Verify initial state
        assert_eq!(game_state.current_turn, PlayerColor::White);
        assert_eq!(game_state.status, GameStatus::WaitingForOpponent);
        assert!(game_state.moves.is_empty());
        assert!(game_state.captured_pieces.is_empty());
    }
    
    #[test]
    fn test_game_state_transitions() {
        let mut state = GameState::new();
        
        // Test initial state
        assert_eq!(state.status, GameStatus::WaitingForOpponent);
        
        // Test transition to active
        state.transition_to_active();
        assert_eq!(state.status, GameStatus::Active);
        
        // Test transition to completed
        state.transition_to_completed(PlayerColor::White);
        assert_eq!(state.status, GameStatus::Completed);
    }
}
```

## Example: Piece Movement Test

```rust
#[test]
fn test_pawn_movement() {
    let board = Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    let pawn_square = Square::from_coords(6, 1); // e2
    
    let moves = get_pseudo_legal_moves(board, pawn_square);
    assert!(moves.len() > 0);
    
    // Check forward move
    let forward_move = ChessMove::new(pawn_square, Square::from_coords(6, 2), None);
    assert!(moves.contains(&forward_move));
}
```
