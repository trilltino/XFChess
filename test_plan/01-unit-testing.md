# Unit Testing Fundamentals

## Rust Unit Test Basics

### Test Structure

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_name() {
        // Arrange
        let input = setup_test_data();
        
        // Act
        let result = function_under_test(input);
        
        // Assert
        assert_eq!(result, expected_value);
    }
}
```

### Assertion Macros

```rust
assert!(condition);                           // Boolean check
assert_eq!(left, right);                      // Equality
assert_ne!(left, right);                      // Inequality
assert!(result.is_ok());                      // Result success
assert!(result.is_err());                     // Result failure
assert_eq!(result.unwrap(), expected);        // Unwrap and compare
```

### Custom Error Messages

```rust
assert_eq!(
    player.mana, 10,
    "Player should start with 10 mana, but had {}", 
    player.mana
);
```

## Testing Patterns

### Testing Pure Functions

```rust
// In crates/chess_engine/src/evaluation.rs
pub fn evaluate_position(board: &Board) -> i32 {
    // ... implementation
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_starting_position_is_balanced() {
        let board = Board::starting_position();
        let score = evaluate_position(&board);
        assert_eq!(score, 0, "Starting position should be balanced");
    }

    #[test]
    fn test_white_up_material() {
        let mut board = Board::starting_position();
        board.remove_piece(Square::D7); // Remove black queen
        let score = evaluate_position(&board);
        assert!(score > 800, "White should be winning by ~900 centipawns");
    }
}
```

### Testing Structs with Default

```rust
#[derive(Default)]
pub struct Selection {
    pub selected_entity: Option<Entity>,
    pub possible_moves: Vec<(u8, u8)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_default_is_empty() {
        let selection = Selection::default();
        assert!(selection.selected_entity.is_none());
        assert!(selection.possible_moves.is_empty());
    }
}
```

### Testing Error Conditions

```rust
#[test]
fn test_invalid_move_returns_error() {
    let board = Board::starting_position();
    let result = board.apply_move(Move::new(0, 63)); // Invalid move
    
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ChessEngineError::InvalidMove { .. }
    ));
}
```

### Testing with Setup/Teardown

```rust
struct TestContext {
    board: Board,
    engine: Engine,
}

impl TestContext {
    fn new() -> Self {
        Self {
            board: Board::starting_position(),
            engine: Engine::new(),
        }
    }
}

#[test]
fn test_engine_finds_checkmate() {
    let ctx = TestContext::new();
    // Use ctx.board and ctx.engine
}
```

## Test Organization

### File Structure for chess_engine

```
crates/chess_engine/src/
├── lib.rs
├── api.rs           # Add #[cfg(test)] mod tests at bottom
├── board.rs         # Add tests for board operations
├── evaluation.rs    # Add tests for position scoring
├── move_gen.rs      # Add tests for move generation
├── search.rs        # Add tests for AI search
└── types.rs         # Add tests for data structures
```

### Running Specific Tests

```bash
# Run all tests in chess_engine
cargo test -p chess_engine

# Run specific test
cargo test -p chess_engine test_starting_position

# Run tests matching pattern
cargo test -p chess_engine move_gen

# Show output from passing tests
cargo test -p chess_engine -- --nocapture
```

## Coverage Measurement

```bash
# Install cargo-tarpaulin
cargo install cargo-tarpaulin

# Run coverage
cargo tarpaulin -p chess_engine --out Html

# View report
open tarpaulin-report.html
```
