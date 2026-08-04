// # Chess Engine Core Types - Data Structures for Alpha-Beta Search
//
// ## Overview
//
// This module defines the fundamental data structures that power the chess engine's AI. The design follows
// **cache-oriented programming** principles, using compact representations to maximize CPU cache hits during
// the millions of position evaluations performed during alpha-beta search.
//
// ## The `Game` Structure - Central Engine State
#[cfg(feature = "std")]
use std::sync::Arc;

#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
#[cfg(not(feature = "std"))]
use alloc::sync::Arc;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
//
// The `Game` struct represents the complete state of a chess engine instance, combining:
// 1. **Current position** (`board: [i8; 64]`) - Piece placement
// 2. **Move history** (`move_counter`, castling rights) - Game rules state
// 3. **Precalculated tables** (`rook`, `bishop`, `knight`, etc.) - Move generation caches
// 4. **Transposition table** (`tt: Box<[TTE; 2M]>`) - Position cache for search speedup
// 5. **Search statistics** (`calls`, `cut`, `tte_hit`) - Performance telemetry
//
// ### Why i8 for Board Representation?
//
// The board uses **signed 8-bit integers** where:
// - Positive values (1-6) represent white pieces
// - Negative values (-1 to -6) represent black pieces
// - Zero represents empty squares
//
// This encoding allows several optimizations:
// - **Sign bit = color**: `piece < 0` is black, `piece > 0` is white
// - **Absolute value = type**: `abs(piece)` gives piece ID (1=pawn, 2=knight, etc.)
// - **Single comparison**: Check piece ownership with one CPU instruction
// - **Compact storage**: 64 bytes total (fits in one cache line on most CPUs)
//
// Compare to alternative representations:
// - `Option<(Color, PieceType)>`: 128 bytes (2x cache misses)
// - Bitboards (one per piece type): 96 bytes, requires 6 separate checks
//
// ## The `Move` Structure - AI Decision Output
//
// When the engine computes the best move, it returns a `Move` struct containing:
// - **src/dst**: Source and destination squares (0-63)
// - **score**: Position evaluation in centipawns
// - **state**: Game outcome (playing, checkmate, stalemate)
// - **checkmate_in**: Ply count to forced checkmate (if applicable)
//
// The `score` field represents the **minimax value** - the evaluation assuming both sides play perfectly.
// Positive scores favor white, negative favor black. A score of +300 means white is up a knight's worth of material/position.
//
// ### Checkmate Distance Calculation
//
// When `score > KING_VALUE_DIV_2`, the engine has found a forced checkmate sequence. The `checkmate_in` field
// counts **plies** (half-moves), so `checkmate_in = 5` means white can force mate in 3 full moves (5 half-moves).
//
// ## The `KK` Structure - Compact Move Representation
//
// During move generation, the engine creates hundreds of `KK` structs representing possible moves. The name is
// historical (German "Klein Kombination" - small combination). Each KK contains:
// - **src/dst**: 8-bit square indices (total 16 bits)
// - **score**: Move ordering heuristic (16 bits)
// - **nxt_dir_idx**: Next direction index for sliding pieces (8 bits)
//
// Total size: **40 bits** (5 bytes), allowing efficient cache usage. An array of 200 legal moves fits in 1KB.
//
// The `nxt_dir_idx` field enables **incremental move generation** for bishops/rooks/queens. Instead of
// generating all squares in a direction at once, the engine can pause mid-generation and resume later,
// saving memory when alpha-beta cutoffs occur early.
//
// ## Transposition Table Architecture
//
// The transposition table (`tt: Box<[TTE; 2M]>`) is a **2 million entry hash table** storing previously
// evaluated positions. This is the single most important optimization in chess engines, providing:
// - **50-90% search time reduction** in middlegames
// - **5x speedup** in endgames (more transpositions)
// - **Detection of repeated positions** (threefold repetition rule)
//
// ### Why Box<[TTE]>?
//
// At 2M entries × ~40 bytes/entry ≈ **80MB**, the transposition table is too large for the stack. `Box<[TTE]>`
// allocates it on the heap, ensuring:
// - No stack overflow
// - Single allocation (vs Vec which might reallocate)
// - Cache-line alignment (heap allocator guarantees)
//
// ### TTE Structure (Transposition Table Entry)
//
// Each `TTE` contains 5 slots (`TT_TRY = 5`) using **replacement strategy**:
// 1. Hash position to entry (index = `hash(position) % 2M`)
// 2. If position already stored: return cached evaluation (hit!)
// 3. If entry full: replace lowest-priority slot
// 4. Priority = `search_depth × 10 + recency`
//
// This ensures deep searches aren't overwritten by shallow ones, while still refreshing stale entries.
//
// ## Zobrist Hashing via BitBuffer192
//
// Positions are hashed into 24-byte buffers (`BitBuffer192 = [u8; 24]`) using a simplified **Zobrist scheme**.
// Full Zobrist requires random bitstrings for every (piece, square) combination - this engine uses a compact
// encoding instead, trading perfect hashing for reduced memory (192 bits vs 768 bits for full Zobrist).
//
// The encoding stores:
// - Piece locations (6 bits per piece × max 32 pieces = 192 bits)
// - Turn to move (1 bit)
// - Castling rights (4 bits)
// - En passant square (6 bits)
//
// This produces **~4 billion unique hashes** (2^32), sufficient for practical play where collisions occur after
// searching ~65,000 positions (birthday paradox). Debugging builds use 32 bytes to include full hash verification.
//
// XFChess integration lives in the game client: `src/engine/board_state.rs`
// (ECS↔engine sync) and `src/game/ai/` (async search tasks).

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
/// Position hash. Historically a 24/32-byte buffer (hence the name, kept for
/// diff-friendliness); now a standard 64-bit Zobrist key.
pub type BitBuffer192 = u64;
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
    pub en_passant_target: Option<i8>,
    pub halfmove_clock: u32,
    pub secs_per_move: f32,

    pub rook: [KKS; 64],
    pub bishop: [KKS; 64],
    pub knight: [KKS; 64],
    pub king: [KKS; 64],
    pub white_pawn: [KKS; 64],
    pub black_pawn: [KKS; 64],

    /// Transposition table. Owned exclusively by whichever thread runs the search —
    /// no mutex needed because the search is always single-threaded per Game instance.
    #[cfg(feature = "search")]
    pub tt: Vec<TTE>,
    /// Number of TT buckets (always a power of two for fast modulo).
    #[cfg(feature = "search")]
    pub tt_capacity: usize,

    /// Zobrist random bitstrings for O(1) incremental hashing.
    #[cfg(feature = "search")]
    pub zobrist_table: [[u64; 64]; 12],
    /// Zobrist bitstring to XOR when it's black's turn.
    #[cfg(feature = "search")]
    pub zobrist_black_turn: u64,
    /// The current position's hash, updated incrementally during search.
    #[cfg(feature = "search")]
    pub current_hash: BitBuffer192,

    // Bitboards for fast move generation (O(1) lookups vs O(64) scans)
    pub white_pawns: BitSet,
    pub white_knights: BitSet,
    pub white_bishops: BitSet,
    pub white_rooks: BitSet,
    pub white_queens: BitSet,
    pub white_kings: BitSet,
    pub black_pawns: BitSet,
    pub black_knights: BitSet,
    pub black_bishops: BitSet,
    pub black_rooks: BitSet,
    pub black_queens: BitSet,
    pub black_kings: BitSet,
    pub occupied_white: BitSet,
    pub occupied_black: BitSet,
    pub occupied: BitSet,

    pub max_depth_so_far: i64,
    pub abs_max_depth: i64,
    pub calls: i64,
    pub cut: i64,
    pub tte_hit: i64,
    pub tte_put: i64,
    pub tte_miss: i64,
    pub cache_size_bytes: usize,

    // Search heuristics
    /// Killer moves: moves that caused a beta cutoff at a given depth
    #[cfg(feature = "search")]
    pub killer_moves: [[Option<KK>; 2]; MAX_DEPTH + 1],
    /// History heuristic: bonus for moves that have caused cutoffs across the search
    #[cfg(feature = "search")]
    pub history_table: [[u32; 64]; 64],
    /// Capture history: [piece_type][to][captured_piece_type]
    #[cfg(feature = "search")]
    pub cap_history: [[[i32; 7]; 64]; 7],
    /// Continuation history: [piece_type][to] scores indexed by previous move
    #[cfg(feature = "search")]
    pub conthist: [[i32; 64]; 64],
    /// Precomputed bitboards for sliding pieces in each direction
    pub sliding_attack_masks: [[BitSet; 8]; 64],
    /// Atomic flag to abort the search (e.g. on timeout)
    #[cfg(feature = "search")]
    pub abort_search: Arc<core::sync::atomic::AtomicBool>,
    /// Static eval per ply, for the `improving` heuristic (indexed by ply, capped at 128).
    #[cfg(feature = "search")]
    pub eval_stack: [i16; 128],
    /// Zobrist hashes of every position on the game + current search path.
    /// `do_move`/`make_move` push, `unmake_move` pops; used for repetition detection.
    #[cfg(feature = "search")]
    pub hash_history: Vec<BitBuffer192>,
    /// Hard wall-clock deadline for the running search — polled inside the
    /// node loop so a long iteration cannot blow the clock.
    #[cfg(feature = "search")]
    pub search_deadline: Option<std::time::Instant>,
}

#[derive(Debug, Clone, Copy)]
pub struct Move {
    pub src: i64,
    pub dst: i64,
    pub score: i64,
    pub state: i32,
    pub checkmate_in: i64,
    pub promo: i8, // 5=Q, 4=R, 3=B, 2=N
}

impl Default for Move {
    fn default() -> Self {
        Move {
            src: 0,
            dst: 0,
            score: LOWEST_SCORE,
            state: STATE_PLAYING,
            checkmate_in: 0,
            promo: 0,
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

/// TT entry bound type: EXACT score, UPPER (all moves failed low), or LOWER (beta cutoff).
pub const TT_EXACT: u8 = 0;
pub const TT_UPPER: u8 = 1;
pub const TT_LOWER: u8 = 2;

#[derive(Copy, Clone)]
pub struct Guide1 {
    pub ply: i64,
    pub score: i16,
    pub best_move_src: i8,
    pub best_move_dst: i8,
    pub best_move_nxt_dir_idx: u8,
    pub bound_type: u8,
}

impl Default for Guide1 {
    fn default() -> Self {
        Guide1 {
            ply: 0,
            score: INVALID_SCORE,
            best_move_src: 0,
            best_move_dst: 0,
            best_move_nxt_dir_idx: 0,
            bound_type: TT_EXACT,
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
            key: 0,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_move_default() {
        let m = Move::default();
        assert_eq!(m.src, 0);
        assert_eq!(m.dst, 0);
        assert_eq!(m.score, LOWEST_SCORE);
        assert_eq!(m.state, STATE_PLAYING);
        assert_eq!(m.checkmate_in, 0);
    }

    #[test]
    fn test_move_is_copy() {
        let m1 = Move {
            src: 12,
            dst: 28,
            score: 100,
            state: STATE_PLAYING,
            checkmate_in: 0,
            promo: 0,
        };
        let m2 = m1; // Copy
        assert_eq!(m1.src, m2.src);
        assert_eq!(m1.dst, m2.dst);
    }

    #[test]
    fn test_kk_default() {
        let kk = KK::default();
        assert_eq!(kk.score, 0);
        assert_eq!(kk.src, 0);
        assert_eq!(kk.dst, 0);
        assert_eq!(kk.nxt_dir_idx, 0);
    }

    #[test]
    fn test_kk_new() {
        let kk = KK::new(12, 28, 100, 0);
        assert_eq!(kk.src, 12);
        assert_eq!(kk.dst, 28);
        assert_eq!(kk.score, 100);
        assert_eq!(kk.nxt_dir_idx, 0);
    }

    #[test]
    fn test_kk_equality() {
        let kk1 = KK::new(12, 28, 100, 0);
        let kk2 = KK::new(12, 28, 100, 0);
        let kk3 = KK::new(12, 29, 100, 0);

        assert_eq!(kk1, kk2);
        assert_ne!(kk1, kk3);
    }

    #[test]
    fn test_guide1_default() {
        let g = Guide1::default();
        assert_eq!(g.ply, 0);
        assert_eq!(g.score, INVALID_SCORE);
        assert_eq!(g.best_move_src, 0);
        assert_eq!(g.best_move_dst, 0);
    }

    #[test]
    fn test_guide2_default() {
        let g = Guide2::default();
        assert_eq!(g.key, 0);
        assert_eq!(g.pri, 0);
    }

    #[test]
    fn test_hash_result_default() {
        let hr = HashResult::default();
        assert_eq!(hr.depth, 0);
        assert_eq!(hr.hit, 0);
    }

    #[test]
    fn test_tte_default() {
        let tte = TTE::default();
        assert_eq!(tte.h.len(), TT_TRY);
    }

    #[test]
    fn test_position_type_alias() {
        let pos: Position = 28; // e4
        assert_eq!(pos, 28i8);
    }

    #[test]
    fn test_color_type_alias() {
        let white: Color = COLOR_WHITE;
        let black: Color = COLOR_BLACK;
        assert!(white > 0);
        assert!(black < 0);
    }
}
