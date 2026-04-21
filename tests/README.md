# Tests

Test suite for XFChess covering core functionality, game flow, and integration tests.

## Overview

The test suite ensures the reliability and correctness of XFChess functionality. Tests are organized into:
- **Component tests** - Unit tests for individual components
- **Integration tests** - End-to-end tests for complete workflows
- **Game flow tests** - Tests for game state transitions
- **Resource tests** - Tests using mock data and fixtures

## Running Tests

XFChess uses Rust's built-in test framework. Run tests with:

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test module
cargo test components

# Run specific test
cargo test test_piece_movement

# Run tests in release mode
cargo test --release

# Run tests with specific features
cargo test --features "bevy"
```

## Test Structure

```
tests/
├── components/       # Component-level unit tests
│   ├── mod.rs
│   ├── game_state_tests.rs
│   └── piece_tests.rs
├── resources/        # Test fixtures and mock data
│   ├── captured_tests.rs
│   ├── engine_tests.rs
│   ├── history_tests.rs
│   └── mod.rs
├── core_tests.rs     # Core functionality tests
├── game_flow_tests.rs # Game flow and state transition tests
└── integration_rollup.rs # Integration tests
```

## Example: Component Test

This example shows a component test for game state validation.

```rust
#[cfg(test)]
mod game_state_tests {
    use super::*;
    use crate::game::state::GameState;
    
    #[test]
    fn test_initial_game_state() {
        let game_state = GameState::new();
        
        // Verify initial state
        assert_eq!(game_state.current_turn, PlayerColor::White);
        assert_eq!(game_state.status, GameStatus::WaitingForOpponent);
        assert!(game_state.moves.is_empty());
    }
    
    #[test]
    fn test_valid_move_updates_state() {
        let mut game_state = GameState::new();
        let move_data = MoveData {
            from: (4, 1), // e2
            to: (4, 3),   // e4
            promotion: None,
        };
        
        // Execute move
        game_state.make_move(move_data).unwrap();
        
        // Verify state update
        assert_eq!(game_state.current_turn, PlayerColor::Black);
        assert_eq!(game_state.moves.len(), 1);
    }
    
    #[test]
    fn test_invalid_move_returns_error() {
        let mut game_state = GameState::new();
        let invalid_move = MoveData {
            from: (4, 1), // e2
            to: (4, 5),   // e6 - invalid for pawn
            promotion: None,
        };
        
        // Verify error
        assert!(game_state.make_move(invalid_move).is_err());
    }
}
```

## Example: Integration Test

This example shows an integration test for tournament registration flow.

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use backend::signing::tournament_store::TournamentStore;
    use solana_sdk::pubkey::Pubkey;
    
    #[tokio::test]
    async fn test_tournament_registration_flow() {
        // Initialize tournament store
        let store = TournamentStore::new();
        
        // Create tournament
        let tournament_id = store
            .create_tournament("Test Tournament".to_string(), 1_000_000_000)
            .await
            .unwrap();
        
        // Verify tournament exists
        let tournament = store.get_tournament(tournament_id).await.unwrap();
        assert_eq!(tournament.name, "Test Tournament");
        assert_eq!(tournament.status, TournamentStatus::Open);
        
        // Register player
        let player_pubkey = Pubkey::new_unique();
        store.add_player(tournament_id, player_pubkey).await.unwrap();
        
        // Verify player registered
        let tournament = store.get_tournament(tournament_id).await.unwrap();
        assert_eq!(tournament.player_count, 1);
    }
}
```
