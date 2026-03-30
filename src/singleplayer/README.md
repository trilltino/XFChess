# Singleplayer Module

## Purpose

The Singleplayer module manages local gameplay against AI opponents in XFChess. It handles AI move generation, difficulty levels, and single-player game state without network connectivity.

## Impact on Game

This module enables:
- **AI Opponents**: Play against computer-controlled opponents
- **Difficulty Levels**: Adjustable AI strength from beginner to master
- **Stockfish Integration**: World-class chess engine for challenging play
- **Offline Play**: Full gameplay without internet connection
- **Practice Mode**: Perfect for learning and skill development

## Architecture/Key Components

### Singleplayer Plugin

| Component | Purpose |
|-----------|---------|
| [`SingleplayerPlugin`](mod.rs:5) | Registers all singleplayer systems |

### AI Integration

The AI functionality is provided through external crates:

| Crate | Purpose |
|-------|---------|
| [`braid_stockfish_ai`](../../crates/braid_stockfish_ai/README.md) | Stockfish UCI integration for move generation |
| [`chess_engine`](../../crates/chess_engine/README.md) | Position validation and FEN handling |

### Difficulty Levels

| Level | ELO Range | Description |
|-------|-----------|-------------|
| Beginner | 400-800 | Limited search depth, random mistakes |
| Easy | 800-1200 | Basic tactical awareness |
| Medium | 1200-1600 | Solid positional play |
| Hard | 1600-2000 | Strong tactical calculations |
| Expert | 2000+ | Near-optimal play |

## Usage

### Starting a Singleplayer Game

```rust
fn start_singleplayer_game(
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
) {
    // Setup AI opponent
    commands.insert_resource(AIOpponent {
        difficulty: Difficulty::Medium,
        engine: StockfishEngine::new(),
    });
    
    next_state.set(AppState::Game);
}
```

### Processing AI Moves

```rust
fn ai_turn(
    mut ai: ResMut<AIOpponent>,
    engine: Res<ChessEngine>,
    mut events: EventWriter<MakeMove>,
) {
    if let Some(best_move) = ai.engine.get_best_move(
        &engine.fen,
        ai.difficulty.search_depth(),
    ) {
        events.send(MakeMove {
            from: best_move.from,
            to: best_move.to,
        });
    }
}
```

### Adjusting Difficulty

```rust
fn set_difficulty(
    mut ai: ResMut<AIOpponent>,
    level: Difficulty,
) {
    ai.difficulty = level;
    // Adjust search depth and evaluation parameters
    ai.engine.set_skill_level(level.skill_level());
}
```

## Dependencies

- [`braid_stockfish_ai`](../../crates/braid_stockfish_ai/README.md) - Stockfish integration
- [`chess_engine`](../../crates/chess_engine/README.md) - Move validation
- [`bevy`](https://docs.rs/bevy) - ECS framework

## Related Modules

- [`engine`](../engine/README.md) - Position validation
- [`game`](../game/README.md) - Game state management
- [`assets`](../assets/README.md) - Piece visualization

## Stockfish Integration

The AI uses Stockfish, one of the strongest chess engines in the world:

```
Game Position (FEN) → Stockfish UCI → Best Move → Validation → Execution
```

### Configuration

- **Search Depth**: Controls AI strength vs. response time
- **Think Time**: Maximum time for move calculation
- **Threads**: CPU cores allocated to engine
- **Hash Size**: Transposition table size

## Performance Considerations

- AI moves are computed asynchronously to prevent frame drops
- Search depth is limited based on difficulty
- Position evaluation is cached when possible
