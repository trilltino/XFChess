# Test Resources

Test fixtures and mock data for XFChess tests, providing reusable test data and utilities.

## Overview

Test resources provide reusable test fixtures, mock data, and helper functions that support test execution across the test suite. These resources:
- Provide consistent test data
- Reduce test code duplication
- Enable realistic test scenarios
- Support complex test setups

## Resource Structure

```
tests/resources/
├── mod.rs               # Module declaration
├── captured_tests.rs    # Piece capture tests
├── engine_tests.rs      # Chess engine tests
└── history_tests.rs     # Move history tests
```

## Example: Test Fixtures

This example shows how to create and use test fixtures.

```rust
#[cfg(test)]
mod fixtures {
    use crate::game::state::GameState;
    
    /// Creates a test game state with a known FEN position
    pub fn create_test_game_state(fen: &str) -> GameState {
        GameState::from_fen(fen).unwrap_or_else(|e| {
            panic!("Failed to create game state from FEN: {}", e);
        })
    }
    
    /// Returns the initial board FEN string
    pub fn initial_board_fen() -> String {
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string()
    }
    
    pub fn mid_game_fen() -> String {
        "r1bqkbnr/pppp1ppp/2n5/1B2p3/4P3/5N2/PPPP1PPP/RNBQK2R b KQkq - 3 3".to_string()
    }
    
    pub fn end_game_fen() -> String {
        "6k1/5ppp/8/8/8/8/5PPP/4R1K1 w - - 0 1".to_string()
    }
```

## Example: Mock Tournament Data

```rust
pub fn mock_tournament() -> Tournament {
    Tournament {
        id: 1,
        name: "Test Tournament".to_string(),
        entry_fee: 1000,
        prize_pool: 4000,
        status: TournamentStatus::Open,
        player_count: 0,
        round: 0,
        ..Default::default()
    }
}
```

## Example: Mock Game State

```rust
pub fn mock_active_game() -> Game {
    let mut game = Game::new();
    game.status = GameStatus::Active;
    game.player_white = Pubkey::new_unique();
    game.player_black = Pubkey::new_unique();
    game.fen = initial_board_fen();
    game
}
```
