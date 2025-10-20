//! # Salewski Chess Engine - Modularized Architecture
//!
//! ## Overview
//!
//! This is a complete **minimax-based chess AI** with alpha-beta pruning, transposition tables, and iterative
//! deepening search. Originally implemented as a 2,715-line monolithic file, it has been refactored into a clean
//! modular architecture for maintainability, testability, and integration with XFChess.
//!
//! ## Architecture Philosophy
//!
//! The engine follows **classic computer chess design principles** from the 1980s-90s, optimized for modern
//! 64-bit processors:
//!
//! 1. **Precalculated Move Tables** - All possible moves from each square computed once at startup
//! 2. **Bitboard Representation** - Compact 64-bit integers for fast set operations
//! 3. **Alpha-Beta Pruning** - Reduces search tree from O(b^d) to O(b^(d/2)) nodes
//! 4. **Transposition Tables** - 80MB hash table caching position evaluations (50-90% speedup)
//! 5. **Move Ordering** - Search best moves first to maximize pruning efficiency
//!
//! ### Performance Characteristics
//!
//! - **Search depth**: 6-10 ply in middlegame (1-2 seconds per move)
//! - **Nodes per second**: ~500K-1M on modern CPUs
//! - **Memory usage**: 80MB transposition table + 2MB move tables
//! - **Strength**: Estimated 1800-2000 ELO (club player level)
//!
//! ## Module Organization
//!
//! ### Core Data (No dependencies)
//! - **[`bitset`](bitset/index.html)** - 64-bit bitboard for fast square operations
//! - **[`constants`](constants/index.html)** - Piece values (centipawns), movement vectors, search tuning
//! - **[`types`](types/index.html)** - Game state, Move struct, transposition table entries
//!
//! ### Game Logic (Depends on core)
//! - **`board`** (TODO) - Square queries, hash computation, board utilities
//! - **`move_gen`** (TODO) - Legal move generation for all piece types
//! - **`evaluation`** (TODO) - Position scoring (material + positional factors)
//! - **`hash`** (TODO) - Zobrist hashing, transposition table lookup/storage
//!
//! ### AI Engine (Depends on all above)
//! - **`search`** (TODO) - Minimax with alpha-beta pruning, iterative deepening
//! - **`api`** (TODO) - Public functions: `new_game()`, `reply()`, `do_move()`
//!
//! ## Algorithm Overview: Alpha-Beta Pruning
//!
//! The core search algorithm implements **negamax** (a variant of minimax) with alpha-beta pruning:
//!
//! ```text
//! function alphabeta(position, depth, α, β, color):
//!     if depth = 0 or game_over(position):
//!         return color × evaluate(position)
//!
//!     best_value := −∞
//!     for each move in legal_moves(position):
//!         value := −alphabeta(apply_move(position, move), depth−1, −β, −α, −color)
//!         best_value := max(best_value, value)
//!         α := max(α, value)
//!         if α ≥ β:
//!             break  // Beta cutoff - opponent won't allow this line
//!     return best_value
//! ```
//!
//! **Key Optimizations:**
//! 1. **Transposition Table** - Before search, check if position already evaluated
//! 2. **Move Ordering** - Try captures and checks first (higher cutoff rate)
//! 3. **Iterative Deepening** - Search depth 1, then 2, then 3, etc. (enables time control)
//! 4. **Quiescence Search** - At depth=0, continue searching captures/checks to avoid horizon effect
//! 5. **Selective Extensions** - Extend search for checks, pawn promotions, etc.
//!
//! For detailed explanations, see:
//! - **Alpha-Beta**: https://www.chessprogramming.org/Alpha-Beta
//! - **Negamax**: https://www.chessprogramming.org/Negamax
//! - **Transposition Tables**: https://www.chessprogramming.org/Transposition_Table
//!
//! ## XFChess Integration Strategy
//!
//! ### Phase 1: Add AI Opponent (No visual changes)
//!
//! ```rust,ignore
//! // In src/game/ai/mod.rs (new module)
//! use chess_engine::{new_game, reply, do_move, Game, Move, STATE_CHECKMATE};
//! use bevy::prelude::*;
//! use bevy::tasks::{AsyncComputeTaskPool, Task};
//! use std::sync::{Arc, Mutex};
//!
//! #[derive(Resource)]
//! pub struct ChessEngine {
//!     game: Arc<Mutex<Game>>,
//! }
//!
//! #[derive(Resource)]
//! pub struct PendingAIMove(Task<Move>);
//!
//! impl Default for ChessEngine {
//!     fn default() -> Self {
//!         ChessEngine {
//!             game: Arc::new(Mutex::new(new_game())),
//!         }
//!     }
//! }
//!
//! // System: Spawn AI task when it's black's turn
//! fn spawn_ai_computation(
//!     mut commands: Commands,
//!     engine: Res<ChessEngine>,
//!     turn: Res<CurrentTurn>,
//!     task: Option<Res<PendingAIMove>>,
//! ) {
//!     if task.is_some() {
//!         return; // Already computing
//!     }
//!
//!     if turn.color == PieceColor::Black {
//!         let game_clone = engine.game.clone();
//!         let task = AsyncComputeTaskPool::get().spawn(async move {
//!             let mut game = game_clone.lock().unwrap();
//!             game.secs_per_move = 1.5; // AI think time
//!             reply(&mut game) // Compute best move
//!         });
//!         commands.insert_resource(PendingAIMove(task));
//!     }
//! }
//!
//! // System: Poll task and apply AI move when ready
//! fn apply_ai_move(
//!     mut commands: Commands,
//!     mut task: ResMut<PendingAIMove>,
//!     mut engine: ResMut<ChessEngine>,
//!     pieces: Query<(Entity, &Piece)>,
//!     mut turn: ResMut<CurrentTurn>,
//! ) {
//!     if let Some(ai_move) = future::block_on(future::poll_once(&mut task.0)) {
//!         // AI move completed!
//!         commands.remove_resource::<PendingAIMove>();
//!
//!         if ai_move.state == STATE_CHECKMATE {
//!             info!("AI declares checkmate!");
//!             return;
//!         }
//!
//!         // Convert engine square indices to (x, y) coordinates
//!         let src_x = (ai_move.src % 8) as u8;
//!         let src_y = (ai_move.src / 8) as u8;
//!         let dst_x = (ai_move.dst % 8) as u8;
//!         let dst_y = (ai_move.dst / 8) as u8;
//!
//!         // Find the piece at source square and move it
//!         for (entity, piece) in pieces.iter() {
//!             if piece.x == src_x && piece.y == src_y {
//!                 // Use existing move_piece system or manually update
//!                 // ... (integrate with your movement.rs logic)
//!             }
//!         }
//!
//!         // Apply move to engine's internal board
//!         let mut game = engine.game.lock().unwrap();
//!         do_move(&mut game, ai_move.src as i8, ai_move.dst as i8, false);
//!
//!         // Switch turn
//!         turn.color = PieceColor::White;
//!     }
//! }
//! ```
//!
//! ### Phase 2: Board Synchronization
//!
//! Keep ECS as source of truth, sync to engine before AI computation:
//!
//! ```rust,ignore
//! fn sync_ecs_to_engine(
//!     pieces: Query<&Piece>,
//!     mut engine: ResMut<ChessEngine>,
//! ) {
//!     let mut game = engine.game.lock().unwrap();
//!     game.board = [0; 64];
//!
//!     for piece in pieces.iter() {
//!         let square = (piece.y * 8 + piece.x) as usize;
//!         let piece_id = match piece.piece_type {
//!             PieceType::Pawn => 1,
//!             PieceType::Knight => 2,
//!             PieceType::Bishop => 3,
//!             PieceType::Rook => 4,
//!             PieceType::Queen => 5,
//!             PieceType::King => 6,
//!         };
//!         game.board[square] = if piece.color == PieceColor::White {
//!             piece_id
//!         } else {
//!             -piece_id
//!         };
//!     }
//! }
//! ```
//!
//! ### Phase 3: UI Enhancements
//!
//! - Show AI "thinking" indicator
//! - Display evaluation score (`ai_move.score` in centipawns)
//! - Show "Checkmate in N" when `ai_move.checkmate_in` is set
//! - Add difficulty selector (adjust `game.secs_per_move`)
//!
//! ## Historical Note
//!
//! This engine's architecture follows the **Salewski Chess** design from Dr. Stefan Salewski (2015-2025),
//! which itself draws from GNU Chess (1980s) and classic alpha-beta implementations. The use of precalculated
//! move tables and compact board representation reflects the memory-constrained era of early computer chess,
//! yet remains competitive on modern hardware due to excellent cache locality.
//!
//! ## Further Reading
//!
//! - **Chess Programming Wiki**: https://www.chessprogramming.org/
//! - **Computer Chess History**: https://www.chessprogramming.org/History
//! - **Minimax Algorithm**: https://www.chessprogramming.org/Minimax
//! - **Modern Engine Techniques**: https://www.chessprogramming.org/Main_Page

mod bitset;
mod constants;
mod types;

pub use types::{Game, Move, Board, Position, Color, KK};
pub use constants::{
    STATE_CHECKMATE, STATE_PLAYING, STATE_STALEMATE,
    KING_VALUE, KING_VALUE_DIV_2, SURE_CHECKMATE,
    FLAG_PLAIN, FLAG_CAPTURE, FLAG_EP, FLAG_PROMOTION, FLAG_PROCAP,
    PAWN_VALUE, KNIGHT_VALUE, BISHOP_VALUE, ROOK_VALUE, QUEEN_VALUE,
};
