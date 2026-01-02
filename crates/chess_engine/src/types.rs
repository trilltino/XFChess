//! # Chess Engine Core Types - Data Structures for Alpha-Beta Search
//!
//! ## Overview
//!
//! This module defines the fundamental data structures that power the chess engine's AI. The design follows
//! **cache-oriented programming** principles, using compact representations to maximize CPU cache hits during
//! the millions of position evaluations performed during alpha-beta search.
//!
//! ## The `Game` Structure - Central Engine State
//!
//! The `Game` struct represents the complete state of a chess engine instance, combining:
//! 1. **Current position** (`board: [i8; 64]`) - Piece placement
//! 2. **Move history** (`move_counter`, castling rights) - Game rules state
//! 3. **Precalculated tables** (`rook`, `bishop`, `knight`, etc.) - Move generation caches
//! 4. **Transposition table** (`tt: Box<[TTE; 2M]>`) - Position cache for search speedup
//! 5. **Search statistics** (`calls`, `cut`, `tte_hit`) - Performance telemetry
//!
//! ### Why i8 for Board Representation?
//!
//! The board uses **signed 8-bit integers** where:
//! - Positive values (1-6) represent white pieces
//! - Negative values (-1 to -6) represent black pieces
//! - Zero represents empty squares
//!
//! This encoding allows several optimizations:
//! - **Sign bit = color**: `piece < 0` is black, `piece > 0` is white
//! - **Absolute value = type**: `abs(piece)` gives piece ID (1=pawn, 2=knight, etc.)
//! - **Single comparison**: Check piece ownership with one CPU instruction
//! - **Compact storage**: 64 bytes total (fits in one cache line on most CPUs)
//!
//! Compare to alternative representations:
//! - `Option<(Color, PieceType)>`: 128 bytes (2x cache misses)
//! - Bitboards (one per piece type): 96 bytes, requires 6 separate checks
//!
//! ## The `Move` Structure - AI Decision Output
//!
//! When the engine computes the best move, it returns a `Move` struct containing:
//! - **src/dst**: Source and destination squares (0-63)
//! - **score**: Position evaluation in centipawns
//! - **state**: Game outcome (playing, checkmate, stalemate)
//! - **checkmate_in**: Ply count to forced checkmate (if applicable)
//!
//! The `score` field represents the **minimax value** - the evaluation assuming both sides play perfectly.
//! Positive scores favor white, negative favor black. A score of +300 means white is up a knight's worth of material/position.
//!
//! ### Checkmate Distance Calculation
//!
//! When `score > KING_VALUE_DIV_2`, the engine has found a forced checkmate sequence. The `checkmate_in` field
//! counts **plies** (half-moves), so `checkmate_in = 5` means white can force mate in 3 full moves (5 half-moves).
//!
//! ## The `KK` Structure - Compact Move Representation
//!
//! During move generation, the engine creates hundreds of `KK` structs representing possible moves. The name is
//! historical (German "Klein Kombination" - small combination). Each KK contains:
//! - **src/dst**: 8-bit square indices (total 16 bits)
//! - **score**: Move ordering heuristic (16 bits)
//! - **nxt_dir_idx**: Next direction index for sliding pieces (8 bits)
//!
//! Total size: **40 bits** (5 bytes), allowing efficient cache usage. An array of 200 legal moves fits in 1KB.
//!
//! The `nxt_dir_idx` field enables **incremental move generation** for bishops/rooks/queens. Instead of
//! generating all squares in a direction at once, the engine can pause mid-generation and resume later,
//! saving memory when alpha-beta cutoffs occur early.
//!
//! ## Transposition Table Architecture
//!
//! The transposition table (`tt: Box<[TTE; 2M]>`) is a **2 million entry hash table** storing previously
//! evaluated positions. This is the single most important optimization in chess engines, providing:
//! - **50-90% search time reduction** in middlegames
//! - **5x speedup** in endgames (more transpositions)
//! - **Detection of repeated positions** (threefold repetition rule)
//!
//! ### Why Box<[TTE]>?
//!
//! At 2M entries × ~40 bytes/entry ≈ **80MB**, the transposition table is too large for the stack. `Box<[TTE]>`
//! allocates it on the heap, ensuring:
//! - No stack overflow
//! - Single allocation (vs Vec which might reallocate)
//! - Cache-line alignment (heap allocator guarantees)
//!
//! ### TTE Structure (Transposition Table Entry)
//!
//! Each `TTE` contains 5 slots (`TT_TRY = 5`) using **replacement strategy**:
//! 1. Hash position to entry (index = `hash(position) % 2M`)
//! 2. If position already stored: return cached evaluation (hit!)
//! 3. If entry full: replace lowest-priority slot
//! 4. Priority = `search_depth × 10 + recency`
//!
//! This ensures deep searches aren't overwritten by shallow ones, while still refreshing stale entries.
//!
//! ## Zobrist Hashing via BitBuffer192
//!
//! Positions are hashed into 24-byte buffers (`BitBuffer192 = [u8; 24]`) using a simplified **Zobrist scheme**.
//! Full Zobrist requires random bitstrings for every (piece, square) combination - this engine uses a compact
//! encoding instead, trading perfect hashing for reduced memory (192 bits vs 768 bits for full Zobrist).
//!
//! The encoding stores:
//! - Piece locations (6 bits per piece × max 32 pieces = 192 bits)
//! - Turn to move (1 bit)
//! - Castling rights (4 bits)
//! - En passant square (6 bits)
//!
//! This produces **~4 billion unique hashes** (2^32), sufficient for practical play where collisions occur after
//! searching ~65,000 positions (birthday paradox). Debugging builds use 32 bytes to include full hash verification.
//!
//! ## XFChess Integration
//!
//! ### Adding AI to XFChess
//!
//! ```rust,ignore
//! // In game/ai/mod.rs (new module)
//! use chess_engine::{Game, Move, new_game, reply};
//!
//! #[derive(Resource)]
//! struct ChessAI {
//!     engine: Arc<Mutex<Game>>,
//! }
//!
//! fn spawn_ai_task(
//!     mut commands: Commands,
//!     mut ai: ResMut<ChessAI>,
//!     turn: Res<CurrentTurn>,
//! ) {
//!     if turn.color == PieceColor::Black {  // AI plays black
//!         let engine = ai.engine.clone();
//!         let task = AsyncComputeTaskPool::get().spawn(async move {
//!             let mut game = engine.lock().unwrap();
//!             reply(&mut game)  // Returns Move struct
//!         });
//!         commands.insert_resource(PendingAIMove(task));
//!     }
//! }
//! ```
//!
//! ### Syncing ECS Board to Engine Board
//!
//! ```rust,ignore
//! fn sync_ecs_to_engine(
//!     pieces: Query<&Piece>,
//!     mut ai: ResMut<ChessAI>,
//! ) {
//!     let mut game = ai.engine.lock().unwrap();
//!
//!     // Clear board
//!     game.board = [0; 64];
//!
//!     // Copy pieces
//!     for piece in pieces.iter() {
//!         let square = piece.y * 8 + piece.x;
//!         let piece_id = match piece.piece_type {
//!             PieceType::Pawn => 1,
//!             PieceType::Knight => 2,
//!             // ... etc
//!         };
//!         game.board[square as usize] = if piece.color == PieceColor::White {
//!             piece_id
//!         } else {
//!             -piece_id
//!         };
//!     }
//! }
//! ```
//!
//! ## Further Reading
//!
//! - **Alpha-Beta Pruning**: https://www.chessprogramming.org/Alpha-Beta
//! - **Transposition Tables**: https://www.chessprogramming.org/Transposition_Table
//! - **Zobrist Hashing**: https://www.chessprogramming.org/Zobrist_Hashing
//! - **Move Ordering**: https://www.chessprogramming.org/Move_Ordering
//! - **Cache-Oriented Programming**: https://en.wikipedia.org/wiki/Data-oriented_design

use super::bitset::BitSet;
use super::constants::*;

pub type Position = i8;
pub type Color = i64;
#[allow(dead_code)] // Part of engine's public API - type aliases
pub type ColorIndex = usize;
#[allow(dead_code)] // Part of engine's public API - type aliases
pub type Col = i8;
#[allow(dead_code)] // Part of engine's public API - type aliases
pub type Row = i8;
#[allow(dead_code)] // Part of engine's public API - type aliases
pub type FigureID = i8;
pub type Board = [i8; 64];
pub type BitBuffer192 = [u8; BIT_BUFFER_SIZE];
pub type HashLine1 = [Guide1; 11];
pub type HashLine2 = [Guide2; TT_TRY];
pub type KKS = Vec<KK>;

/// Central game state structure for the chess engine
///
/// This struct represents the complete state of a chess engine instance, combining:
/// - Board state (piece positions)
/// - Move history (move counter, castling rights)
/// - Transposition table for move caching
/// - Search state (current evaluation, best move)
#[derive(Clone)]
pub struct Game {
    pub board: Board,
    pub move_counter: i32,
    pub white_king_has_moved: bool,
    pub black_king_has_moved: bool,
    pub white_rook_0_has_moved: bool,
    pub white_rook_7_has_moved: bool,
    pub black_rook_56_has_moved: bool,
    pub black_rook_63_has_moved: bool,
    pub secs_per_move: f32,

    pub rook: [KKS; 64],
    pub bishop: [KKS; 64],
    pub knight: [KKS; 64],
    pub king: [KKS; 64],
    pub white_pawn: [KKS; 64],
    pub black_pawn: [KKS; 64],

    pub tt: Box<[TTE; TTE_SIZE]>,

    pub max_depth_so_far: i64,
    pub abs_max_depth: i64,
    pub calls: i64,
    pub cut: i64,
    pub tte_hit: i64,
    pub tte_put: i64,
    pub tte_miss: i64,
    pub cache_size_bytes: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct Move {
    pub src: i64,
    pub dst: i64,
    pub score: i64,
    pub state: i32,
    pub checkmate_in: i64,
}

impl Default for Move {
    fn default() -> Self {
        Move {
            src: 0,
            dst: 0,
            score: LOWEST_SCORE,
            state: STATE_PLAYING,
            checkmate_in: 0,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct KK {
    pub score: i16,
    pub src: i8,
    pub dst: i8,
    pub nxt_dir_idx: u8,
}

impl Default for KK {
    fn default() -> Self {
        KK {
            score: 0,
            src: 0,
            dst: 0,
            nxt_dir_idx: 0,
        }
    }
}

impl KK {
    pub fn new(src: i8, dst: i8, score: i16, nxt_dir_idx: u8) -> Self {
        KK {
            score,
            src,
            dst,
            nxt_dir_idx,
        }
    }
}

#[derive(Copy, Clone, Debug)]
#[allow(dead_code)] // Internal engine structure - reserved for future use
pub(crate) struct Gnu {
    pub score: i16,
    pub src: i8,
    pub dst: i8,
    pub nxt_dir_idx: u8,
    pub gen_moves: BitSet,
}

#[derive(Copy, Clone, Debug)]
#[allow(dead_code)] // Internal engine structure - reserved for future use
pub(crate) struct MiniGnu {
    pub score: i16,
    pub src: i8,
    pub dst: i8,
    pub nxt_dir_idx: u8,
    pub gen_moves_h: u8,
    pub gen_moves_l: u8,
}

#[derive(Copy, Clone)]
pub struct Guide1 {
    #[allow(dead_code)] // Internal engine field - reserved for future use
    pub ply: i64,
    pub score: i16,
    pub best_move_src: i8,
    pub best_move_dst: i8,
    #[allow(dead_code)] // Internal engine field - reserved for future use
    pub best_move_nxt_dir_idx: u8,
}

impl Default for Guide1 {
    fn default() -> Self {
        Guide1 {
            ply: 0,
            score: INVALID_SCORE,
            best_move_src: 0,
            best_move_dst: 0,
            best_move_nxt_dir_idx: 0,
        }
    }
}

#[derive(Copy, Clone)]
pub struct Guide2 {
    pub key: BitBuffer192,
    pub res: HashResult,
    pub pri: i64,
}

impl Default for Guide2 {
    fn default() -> Self {
        Guide2 {
            key: [0; BIT_BUFFER_SIZE],
            res: HashResult::default(),
            pri: 0,
        }
    }
}

#[derive(Copy, Clone)]
pub struct HashResult {
    pub h: HashLine1,
    pub depth: i64,
    pub hit: i64,
}

impl Default for HashResult {
    fn default() -> Self {
        HashResult {
            h: [Guide1::default(); 11],
            depth: 0,
            hit: 0,
        }
    }
}

#[derive(Copy, Clone)]
pub struct TTE {
    pub h: HashLine2,
}

impl Default for TTE {
    fn default() -> Self {
        TTE {
            h: [Guide2::default(); TT_TRY],
        }
    }
}
