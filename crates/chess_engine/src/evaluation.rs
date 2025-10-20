//! Position evaluation with piece-square tables
//!
//! Evaluates chess positions using:
//! - Material count (piece values)
//! - Positional bonuses (piece-square tables)
//! - Mobility (number of legal moves)
//! - King safety

use super::types::*;
use super::constants::*;
use super::move_gen::*;
use super::board::*;

/// Piece-Square Tables for positional evaluation
/// Values are in centipawns, from white's perspective

const PAWN_PST: [i16; 64] = [
     0,  0,  0,  0,  0,  0,  0,  0,
     5, 10, 10,-20,-20, 10, 10,  5,
     5, -5,-10,  0,  0,-10, -5,  5,
     0,  0,  0, 20, 20,  0,  0,  0,
     5,  5, 10, 25, 25, 10,  5,  5,
    10, 10, 20, 30, 30, 20, 10, 10,
    50, 50, 50, 50, 50, 50, 50, 50,
     0,  0,  0,  0,  0,  0,  0,  0,
];

const KNIGHT_PST: [i16; 64] = [
   -50,-40,-30,-30,-30,-30,-40,-50,
   -40,-20,  0,  5,  5,  0,-20,-40,
   -30,  5, 10, 15, 15, 10,  5,-30,
   -30,  0, 15, 20, 20, 15,  0,-30,
   -30,  5, 15, 20, 20, 15,  5,-30,
   -30,  0, 10, 15, 15, 10,  0,-30,
   -40,-20,  0,  0,  0,  0,-20,-40,
   -50,-40,-30,-30,-30,-30,-40,-50,
];

const BISHOP_PST: [i16; 64] = [
   -20,-10,-10,-10,-10,-10,-10,-20,
   -10,  5,  0,  0,  0,  0,  5,-10,
   -10, 10, 10, 10, 10, 10, 10,-10,
   -10,  0, 10, 10, 10, 10,  0,-10,
   -10,  5,  5, 10, 10,  5,  5,-10,
   -10,  0,  5, 10, 10,  5,  0,-10,
   -10,  0,  0,  0,  0,  0,  0,-10,
   -20,-10,-10,-10,-10,-10,-10,-20,
];

const ROOK_PST: [i16; 64] = [
     0,  0,  0,  5,  5,  0,  0,  0,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
     5, 10, 10, 10, 10, 10, 10,  5,
     0,  0,  0,  0,  0,  0,  0,  0,
];

const QUEEN_PST: [i16; 64] = [
   -20,-10,-10, -5, -5,-10,-10,-20,
   -10,  0,  0,  0,  0,  0,  0,-10,
   -10,  5,  5,  5,  5,  5,  0,-10,
     0,  0,  5,  5,  5,  5,  0, -5,
    -5,  0,  5,  5,  5,  5,  0, -5,
   -10,  0,  5,  5,  5,  5,  0,-10,
   -10,  0,  0,  0,  0,  0,  0,-10,
   -20,-10,-10, -5, -5,-10,-10,-20,
];

const KING_PST_MIDDLEGAME: [i16; 64] = [
    20, 30, 10,  0,  0, 10, 30, 20,
    20, 20,  0,  0,  0,  0, 20, 20,
   -10,-20,-20,-20,-20,-20,-20,-10,
   -20,-30,-30,-40,-40,-30,-30,-20,
   -30,-40,-40,-50,-50,-40,-40,-30,
   -30,-40,-40,-50,-50,-40,-40,-30,
   -30,-40,-40,-50,-50,-40,-40,-30,
   -30,-40,-40,-50,-50,-40,-40,-30,
];

/// Get piece-square table value for a piece at a position
fn get_pst_value(piece: i8, square: i8) -> i16 {
    let abs_piece = piece.abs();
    let is_white = piece > 0;

    // Flip square for black pieces (they see the board upside down)
    let pst_index = if is_white {
        square as usize
    } else {
        (63 - square) as usize
    };

    let value = match abs_piece {
        PAWN_ID => PAWN_PST[pst_index],
        KNIGHT_ID => KNIGHT_PST[pst_index],
        BISHOP_ID => BISHOP_PST[pst_index],
        ROOK_ID => ROOK_PST[pst_index],
        QUEEN_ID => QUEEN_PST[pst_index],
        KING_ID => KING_PST_MIDDLEGAME[pst_index],
        _ => 0,
    };

    if is_white { value } else { -value }
}

/// Evaluate material balance
pub fn evaluate_material(game: &Game) -> i16 {
    let mut score = 0i16;

    for square in 0..64 {
        let piece = game.board[square];
        if piece != 0 {
            let piece_value = FIGURE_VALUE[piece.abs() as usize];
            score += if piece > 0 {
                piece_value
            } else {
                -piece_value
            };
        }
    }

    score
}

/// Evaluate full position (material + positional)
pub fn evaluate_position(game: &Game) -> i16 {
    let mut score = 0i16;

    // Material and piece-square tables
    for square in 0..64 {
        let piece = game.board[square];
        if piece != 0 {
            // Material
            let piece_value = FIGURE_VALUE[piece.abs() as usize];
            score += if piece > 0 { piece_value } else { -piece_value };

            // Positional
            let pst_value = get_pst_value(piece, square as i8);
            score += pst_value;
        }
    }

    // Mobility bonus (simplified)
    let white_moves = count_moves(game, COLOR_WHITE);
    let black_moves = count_moves(game, COLOR_BLACK);
    score += (white_moves as i16 - black_moves as i16) * 5;

    score
}

/// Count number of pseudo-legal moves for a color
fn count_moves(game: &Game, color: Color) -> usize {
    generate_pseudo_legal_moves(game, color).len()
}

/// Check if position is endgame (few pieces remaining)
#[allow(dead_code)]
pub fn is_endgame(game: &Game) -> bool {
    let mut piece_count = 0;
    let mut queen_count = 0;

    for square in 0..64 {
        let piece = game.board[square].abs();
        if piece != 0 && piece != KING_ID {
            piece_count += 1;
            if piece == QUEEN_ID {
                queen_count += 1;
            }
        }
    }

    // Endgame if queens are off or very few pieces
    queen_count == 0 || piece_count <= 6
}
