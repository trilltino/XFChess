//! A minimax chess AI with alpha-beta pruning, transposition tables, and
//! iterative deepening search. One crate serves two builds: the full `std`
//! search engine used by the game client and backend, and a `no_std`
//! move-generation-only subset (`on_chain*` modules) that gets compiled into
//! the Solana program via `chess-logic-on-chain`. See the crate README for
//! the module map and the `std`/`no_std` feature boundary.
//!
//! ## Architecture Philosophy
//!
//! The engine follows classic computer chess design principles from the
//! 1980s-90s, optimized for modern 64-bit processors:
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
//! ## XFChess Integration
//!
//! The game client drives this engine from `src/game/ai/` (`ChessAIResource` in
//! `src/game/ai/resource.rs`, systems in `src/game/ai/systems.rs`): it holds the
//! engine's `Game` state, spawns AI move computation as a Bevy async task keyed
//! off the current turn, and applies the resulting `Move` back onto the ECS
//! board once the task completes.
//!
//! ## Further Reading
//!
//! - **Chess Programming Wiki**: https://www.chessprogramming.org/
//! - **Computer Chess History**: https://www.chessprogramming.org/History
//! - **Minimax Algorithm**: https://www.chessprogramming.org/Minimax
//! - **Modern Engine Techniques**: https://www.chessprogramming.org/Main_Page

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod api;
mod bitset;
mod board;
mod constants;
mod error;
mod evaluation;
#[cfg(feature = "search")]
mod hash;
pub mod move_gen;
#[cfg(feature = "search")]
pub mod perft;
#[cfg(feature = "search")]
mod search;
#[cfg(feature = "search")]
mod see;
pub mod types;
mod utils;

// --- On-chain validation path (no std, no alloc, no search tables) ---
pub mod on_chain;
pub mod on_chain_attack;
pub mod on_chain_moves;

#[cfg(feature = "std")]
mod pgn;

#[cfg(feature = "std")]
pub mod book;

// Re-export public API
#[cfg(feature = "search")]
pub use api::reply;
pub use api::{
    do_move, do_move_with_promo, get_game_state, is_legal_move, is_legal_move_unchecked,
};
#[cfg(feature = "std")]
pub use api::{
    game_from_fen, game_from_fen_no_tt, game_to_fen, new_game, new_game_no_tt, reset_game,
    set_game_from_fen, set_tt_size_mb,
};
pub use error::{ChessEngineError, ChessEngineResult};
#[cfg(feature = "std")]
pub use evaluation::evaluate_position;
pub use move_gen::{generate_pseudo_legal_moves, is_in_check};
#[cfg(feature = "std")]
pub use pgn::{
    move_to_san, parse_pgn, parse_pgn_annotated, san_to_move, MoveQuality, ParsedPgnGame,
    PerPlyAnnotation, PgnAssembler, PgnParseError, PgnResult,
};

// Re-export types
pub use types::{Board, Color, Game, Move, Position, KK};

// Re-export constants
pub use constants::{
    BISHOP_ID, BISHOP_VALUE, COLOR_BLACK, COLOR_WHITE, FLAG_CAPTURE, FLAG_EP, FLAG_PLAIN,
    FLAG_PROCAP, FLAG_PROMOTION, KING_ID, KING_VALUE, KING_VALUE_DIV_2, KNIGHT_ID, KNIGHT_VALUE,
    PAWN_ID, PAWN_VALUE, QUEEN_ID, QUEEN_VALUE, ROOK_ID, ROOK_VALUE, STATE_CHECKMATE,
    STATE_PLAYING, STATE_STALEMATE, SURE_CHECKMATE,
};

// Re-export on-chain validation API
pub use on_chain::{CompactBoard, OnChainGame, CASTLE_BK, CASTLE_BQ, CASTLE_WK, CASTLE_WQ};
pub use on_chain_attack::{bishop_attacks, is_in_check_fast, queen_attacks, rook_attacks};
pub use on_chain_moves::{
    has_any_legal_move, parse_uci, validate_and_apply, validate_and_apply_sq, MoveOutcome,
};
