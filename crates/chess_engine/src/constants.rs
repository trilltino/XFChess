//! # Chess Engine Constants - Evaluation Values & Search Parameters
//!
//! ## Overview
//!
//! This module centralizes all constant values used throughout the chess engine, including piece valuations
//! (in centipawns), movement direction vectors, board coordinate mappings, and alpha-beta search tuning
//! parameters. These constants form the foundation of the engine's evaluation function and are based on
//! decades of computer chess research and empirical testing.
//!
//! ## Centipawn Valuation System
//!
//! Chess engines use **centipawns** (1/100th of a pawn) as the standard unit for position evaluation. This
//! allows fine-grained scoring while maintaining integer arithmetic for performance. The valuation scheme:
//!
//! - **Pawn**: 100 centipawns (reference unit)
//! - **Knight**: 300 centipawns (~3 pawns)
//! - **Bishop**: 300 centipawns (~3 pawns, equal to knight)
//! - **Rook**: 500 centipawns (~5 pawns)
//! - **Queen**: 900 centipawns (~9 pawns)
//! - **King**: 18,000 centipawns (effectively infinite - game-ending)
//!
//! ### Historical Context & Tuning
//!
//! These values come from statistical analysis of millions of chess games and represent:
//! 1. **Material advantage** (intrinsic piece power)
//! 2. **Exchange values** (what trades are favorable)
//! 3. **Compensation** (e.g., queen for 2 rooks is roughly equal)
//!
//! Different engines use slight variations (e.g., Crafty uses 325 for knights/bishops), but this
//! 100/300/300/500/900 scheme is the most common and provides good results. The equal valuation of
//! bishops and knights is deliberately simplified - positional factors (open positions favor bishops,
//! closed positions favor knights) are handled by the evaluation function, not these base values.
//!
//! ### Why 18,000 for King?
//!
//! The king's value is set higher than all other pieces combined (~16 points max) to ensure the engine
//! never considers sacrificing the king. Values above `KING_VALUE_DIV_2` (9,000) trigger checkmate
//! detection, allowing the engine to distinguish between "winning material" and "forcing checkmate."
//!
//! ## Direction Vectors for Move Generation
//!
//! Movement is encoded as offsets in a one-dimensional array representing the board (0-63):
//! - **North (N)**: -8 (move up one rank)
//! - **South (S)**: +8 (move down one rank)
//! - **East (O)**: +1 (move right one file)
//! - **West (W)**: -1 (move left one file)
//! - **Diagonals**: Combinations (NO, NW, SO, SW)
//!
//! This encoding enables **precalculated move generation** where all possible moves for each piece type
//! from each square are computed once at startup and stored in lookup tables (see `move_gen.rs`). For
//! example, a rook on square 35 can reach squares [27, 19, 11, 3] by repeatedly adding -8 (north)
//! until hitting the board edge or another piece.
//!
//! ### Knight Moves: The "L-Shape" Pattern
//!
//! Knights are special because they jump rather than slide. Their 8 possible moves are encoded as:
//! - 2 up + 1 right: N+N+O = -8-8+1 = -15
//! - 2 up + 1 left: N+N+W = -8-8-1 = -17
//! - 2 down + 1 right: S+S+O = +8+8+1 = +17
//! - etc.
//!
//! This compact encoding allows fast iteration over knight destinations without complex branching logic.
//!
//! ## Alpha-Beta Search Extension Flags
//!
//! The constants `CASTLING_EXTEND`, `CHECK_EXTEND`, `PAWN_MARCH_EXTEND`, etc. control when the engine
//! **extends** its search depth beyond the base limit. These are critical tuning parameters that prevent
//! the **horizon effect** (missing tactics just beyond the search depth).
//!
//! ### Extension Philosophy
//!
//! Extensions are enabled for:
//! - **Check** (opponent's king attacked): Must search deeper to find escape/refutation
//! - **Pawn promotion imminent**: A pawn two squares from queening deserves deeper analysis
//! - **Equal trades** (e.g., bishop for bishop): May have tactical implications
//! - **Castling**: Can suddenly change king safety dramatically
//!
//! Extensions are **disabled** for:
//! - **Range of movement**: Longer moves aren't inherently more important
//! - **All captures**: Would explode search time (handled by move ordering instead)
//! - **Large captures** (pawn takes queen): Already scored highly, extension unnecessary
//!
//! This careful tuning prevents search explosion while catching critical tactics. The `V_RATIO` of 8
//! means extending by 1 ply reduces the remaining depth by 1/8, allowing multiple extensions per line
//! without runaway growth.
//!
//! ## XFChess Integration
//!
//! ### Using Piece Values for UI Feedback
//!
//! XFChess can use these constants to show players material advantage:
//!
//! ```rust,ignore
//! // In game/resources/evaluation.rs
//! use chess_engine::constants::{PAWN_VALUE, KNIGHT_VALUE, ROOK_VALUE, QUEEN_VALUE};
//!
//! #[derive(Resource)]
//! pub struct MaterialBalance {
//!     white_material: i16,
//!     black_material: i16,
//! }
//!
//! pub fn calculate_material(pieces: Query<&Piece>) -> MaterialBalance {
//!     let mut balance = MaterialBalance { white_material: 0, black_material: 0 };
//!
//!     for piece in pieces.iter() {
//!         let value = match piece.piece_type {
//!             PieceType::Pawn => PAWN_VALUE,
//!             PieceType::Knight => KNIGHT_VALUE,
//!             PieceType::Bishop => BISHOP_VALUE,
//!             PieceType::Rook => ROOK_VALUE,
//!             PieceType::Queen => QUEEN_VALUE,
//!             PieceType::King => 0, // Don't count king in material
//!         };
//!
//!         match piece.color {
//!             PieceColor::White => balance.white_material += value,
//!             PieceColor::Black => balance.black_material += value,
//!         }
//!     }
//!
//!     balance
//! }
//! ```
//!
//! ### Direction Vectors for Move Validation
//!
//! XFChess's `piece_moves.rs` can use these direction constants for cleaner rook/bishop/queen validation:
//!
//! ```rust,ignore
//! use chess_engine::constants::{ROOK_DIRS, BISHOP_DIRS};
//!
//! fn is_valid_rook_move(from: (u8, u8), to: (u8, u8), board: &BoardState) -> bool {
//!     let from_idx = (from.1 * 8 + from.0) as i32;
//!     let to_idx = (to.1 * 8 + to.0) as i32;
//!
//!     for &dir in &ROOK_DIRS {
//!         let mut pos = from_idx + dir;
//!         while pos >= 0 && pos < 64 {
//!             if pos == to_idx { return true; }
//!             if board.is_occupied(pos) { break; }
//!             pos += dir;
//!         }
//!     }
//!     false
//! }
//! ```
//!
//! ## Further Reading
//!
//! - **Centipawns**: https://www.chessprogramming.org/Centipawns
//! - **Piece Values**: https://www.chess.com/terms/chess-piece-value
//! - **Alpha-Beta Search**: https://www.chessprogramming.org/Alpha-Beta
//! - **Search Extensions**: https://www.chessprogramming.org/Extensions
//! - **Horizon Effect**: https://www.chessprogramming.org/Horizon_Effect

use core::ops::Range;

pub const VOID_ID: i8 = 0;
pub const PAWN_ID: i8 = 1;
pub const KNIGHT_ID: i8 = 2;
pub const BISHOP_ID: i8 = 3;
pub const ROOK_ID: i8 = 4;
pub const QUEEN_ID: i8 = 5;
pub const KING_ID: i8 = 6;
pub const ARRAY_BASE_6: i8 = 6;

pub const W_PAWN: i8 = PAWN_ID;
pub const W_KNIGHT: i8 = KNIGHT_ID;
pub const W_BISHOP: i8 = BISHOP_ID;
pub const W_ROOK: i8 = ROOK_ID;
pub const W_QUEEN: i8 = QUEEN_ID;
pub const W_KING: i8 = KING_ID;

pub const B_PAWN: i8 = -PAWN_ID;
pub const B_KNIGHT: i8 = -KNIGHT_ID;
pub const B_BISHOP: i8 = -BISHOP_ID;
pub const B_ROOK: i8 = -ROOK_ID;
pub const B_QUEEN: i8 = -QUEEN_ID;
pub const B_KING: i8 = -KING_ID;

pub const FORWARD: i32 = 8;
pub const SIDEWARD: i32 = 1;
pub const S: i32 = FORWARD;
pub const O: i32 = SIDEWARD;
pub const N: i32 = -S;
pub const W: i32 = -O;
pub const NO: i32 = N + O;
pub const SO: i32 = S + O;
pub const NW: i32 = N + W;
pub const SW: i32 = S + W;

pub const PAWN_DIRS_WHITE: [i32; 4] = [N, NO, NW, N + N];

pub const BISHOP_DIRS: [i32; 4] = [NO, SO, NW, SW];
pub const ROOK_DIRS: [i32; 4] = [N, O, S, W];
pub const KNIGHT_DIRS: [i32; 8] = [
    N + N + O, N + N + W, S + S + O, S + S + W,
    O + O + N, O + O + S, W + W + N, W + W + S,
];
pub const KING_DIRS: [i32; 8] = [N, O, S, W, NO, SO, NW, SW];

pub const AB_INF: i16 = 32000;
pub const VOID_VALUE: i16 = 0;
pub const PAWN_VALUE: i16 = 100;
pub const KNIGHT_VALUE: i16 = 300;
pub const BISHOP_VALUE: i16 = 300;
pub const ROOK_VALUE: i16 = 500;
pub const QUEEN_VALUE: i16 = 900;
pub const KING_VALUE: i16 = 18000;
pub const KING_VALUE_DIV_2: i16 = KING_VALUE / 2;
pub const SURE_CHECKMATE: i16 = KING_VALUE / 2;

pub const FIGURE_VALUE: [i16; KING_ID as usize + 1] = [
    VOID_VALUE,
    PAWN_VALUE,
    KNIGHT_VALUE,
    BISHOP_VALUE,
    ROOK_VALUE,
    QUEEN_VALUE,
    KING_VALUE,
];

pub const SETUP: [i8; 64] = [
    W_ROOK, W_KNIGHT, W_BISHOP, W_KING, W_QUEEN, W_BISHOP, W_KNIGHT, W_ROOK,
    W_PAWN, W_PAWN, W_PAWN, W_PAWN, W_PAWN, W_PAWN, W_PAWN, W_PAWN,
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
    B_PAWN, B_PAWN, B_PAWN, B_PAWN, B_PAWN, B_PAWN, B_PAWN, B_PAWN,
    B_ROOK, B_KNIGHT, B_BISHOP, B_KING, B_QUEEN, B_BISHOP, B_KNIGHT, B_ROOK,
];

pub const BA: usize = 7;
pub const BB: usize = 6;
pub const BC: usize = 5;
pub const BD: usize = 4;
pub const BE: usize = 3;
pub const BF: usize = 2;
pub const BG: usize = 1;
pub const BH: usize = 0;

pub const B1: usize = 0;
pub const B2: usize = 1;
pub const B3: usize = 2;
pub const B4: usize = 3;
pub const B5: usize = 4;
pub const B6: usize = 5;
pub const B7: usize = 6;
pub const B8: usize = 7;

pub const POS_RANGE: Range<i8> = 0..64;
pub const POS_RANGE_US: Range<usize> = 0..64;

pub const COLOR_BLACK: i64 = -1;
pub const COLOR_WHITE: i64 = 1;

pub const WR0: usize = 0;
pub const WK3: usize = 3;
pub const WR7: usize = 7;
pub const BR56: usize = 56;
pub const BK59: usize = 59;
pub const BR63: usize = 63;

pub const MAX_DEPTH: usize = 15;

pub const IGNORE_MARKER_LOW_INT16: i16 = i16::MIN;
pub const INVALID_SCORE: i16 = i16::MIN;
pub const LOWEST_SCORE: i64 = -i16::MAX as i64;
pub const BETH: i64 = i16::MIN as i64;
pub const NO_NXT_DIR_IDX: u8 = 100;

pub const STATE_PLAYING: i32 = 0;
pub const STATE_STALEMATE: i32 = 1;
pub const STATE_CHECKMATE: i32 = 2;
pub const STATE_NO_VALID_MOVE: i32 = 3;
pub const STATE_CAN_CAPTURE_KING: i32 = 4;

pub const FLAG_PLAIN: i32 = 0;
pub const FLAG_CAPTURE: i32 = 1;
pub const FLAG_EP: i32 = 2;
pub const FLAG_PROMOTION: i32 = 3;
pub const FLAG_PROCAP: i32 = 4;

pub const TTE_SIZE: usize = 1024 * 1024 * 2;
pub const TT_TRY: usize = 5;

pub const CORE_BIT_BUFFER_SIZE: usize = 24;
pub const HASH_BIT_BUFFER_SIZE: usize = 32;
pub const BIT_BUFFER_SIZE: usize = bit_buffer_size();

pub const fn bit_buffer_size() -> usize {
    #[cfg(feature = "salewskiChessDebug")]
    {
        HASH_BIT_BUFFER_SIZE
    }
    #[cfg(not(feature = "salewskiChessDebug"))]
    {
        CORE_BIT_BUFFER_SIZE
    }
}

pub const V_RATIO: i64 = 8;
pub const RANGE_EXTEND: bool = false;
pub const SELECT_EXTEND: bool = false;
pub const CASTLING_EXTEND: bool = true;
pub const CAPTURE_EXTEND: bool = false;
pub const EQUAL_CAPTURE_EXTEND: bool = true;
pub const LARGE_CAPTURE_EXTEND: bool = false;
pub const PAWN_MARCH_EXTEND: bool = true;
pub const CHECK_EXTEND: bool = true;
pub const PROMOTE_EXTEND: bool = true;
pub const NO_EXTEND_AT_ALL: bool = false;

pub const FIG_STR: [&str; 7] = ["  ", "  ", "N_", "B_", "R_", "Q_", "K_"];
