//! Piece-square tables for positional evaluation
//!
//! Piece-square tables (PSTs) provide positional bonuses based on where
//! pieces are located on the board. Values are in centipawns, from white's perspective.

use crate::constants::*;

/// Piece-Square Tables for positional evaluation
/// Values are in centipawns, from white's perspective

pub(crate) const PAWN_PST: [i16; 64] = [
    0, 0, 0, 0, 0, 0, 0, 0, 5, 10, 10, -20, -20, 10, 10, 5, 5, -5, -10, 0, 0, -10, -5, 5, 0, 0, 0,
    20, 20, 0, 0, 0, 5, 5, 10, 25, 25, 10, 5, 5, 10, 10, 20, 30, 30, 20, 10, 10, 50, 50, 50, 50,
    50, 50, 50, 50, 0, 0, 0, 0, 0, 0, 0, 0,
];

pub(crate) const KNIGHT_PST: [i16; 64] = [
    -50, -40, -30, -30, -30, -30, -40, -50, -40, -20, 0, 5, 5, 0, -20, -40, -30, 5, 10, 15, 15, 10,
    5, -30, -30, 0, 15, 20, 20, 15, 0, -30, -30, 5, 15, 20, 20, 15, 5, -30, -30, 0, 10, 15, 15, 10,
    0, -30, -40, -20, 0, 0, 0, 0, -20, -40, -50, -40, -30, -30, -30, -30, -40, -50,
];

pub(crate) const BISHOP_PST: [i16; 64] = [
    -20, -10, -10, -10, -10, -10, -10, -20, -10, 5, 0, 0, 0, 0, 5, -10, -10, 10, 10, 10, 10, 10,
    10, -10, -10, 0, 10, 10, 10, 10, 0, -10, -10, 5, 5, 10, 10, 5, 5, -10, -10, 0, 5, 10, 10, 5, 0,
    -10, -10, 0, 0, 0, 0, 0, 0, -10, -20, -10, -10, -10, -10, -10, -10, -20,
];

pub(crate) const ROOK_PST: [i16; 64] = [
    0, 0, 0, 5, 5, 0, 0, 0, -5, 0, 0, 0, 0, 0, 0, -5, -5, 0, 0, 0, 0, 0, 0, -5, -5, 0, 0, 0, 0, 0,
    0, -5, -5, 0, 0, 0, 0, 0, 0, -5, -5, 0, 0, 0, 0, 0, 0, -5, 5, 10, 10, 10, 10, 10, 10, 5, 0, 0,
    0, 0, 0, 0, 0, 0,
];

pub(crate) const QUEEN_PST: [i16; 64] = [
    -20, -10, -10, -5, -5, -10, -10, -20, -10, 0, 0, 0, 0, 0, 0, -10, -10, 5, 5, 5, 5, 5, 0, -10,
    0, 0, 5, 5, 5, 5, 0, -5, -5, 0, 5, 5, 5, 5, 0, -5, -10, 0, 5, 5, 5, 5, 0, -10, -10, 0, 0, 0, 0,
    0, 0, -10, -20, -10, -10, -5, -5, -10, -10, -20,
];

pub(crate) const KING_PST_MIDDLEGAME: [i16; 64] = [
    20, 30, 10, 0, 0, 10, 30, 20, 20, 20, 0, 0, 0, 0, 20, 20, -10, -20, -20, -20, -20, -20, -20,
    -10, -20, -30, -30, -40, -40, -30, -30, -20, -30, -40, -40, -50, -50, -40, -40, -30, -30, -40,
    -40, -50, -50, -40, -40, -30, -30, -40, -40, -50, -50, -40, -40, -30, -30, -40, -40, -50, -50,
    -40, -40, -30,
];

/// Endgame PSTs — kings centralize, pawns push, pieces become more active
pub(crate) const PAWN_PST_EG: [i16; 64] = [
    0,  0,  0,  0,  0,  0,  0,  0,
   80, 80, 80, 80, 80, 80, 80, 80,
   50, 50, 50, 50, 50, 50, 50, 50,
   30, 30, 30, 30, 30, 30, 30, 30,
   20, 20, 20, 20, 20, 20, 20, 20,
   10, 10, 10, 10, 10, 10, 10, 10,
    0,  0,  0,  0,  0,  0,  0,  0,
    0,  0,  0,  0,  0,  0,  0,  0,
];

pub(crate) const KNIGHT_PST_EG: [i16; 64] = [
    -10, -5, 0, 0, 0, 0, -5, -10, -5, 0, 5, 5, 5, 5, 0, -5, 0, 5, 10, 10, 10, 10, 5, 0, 0, 5, 10,
    15, 15, 10, 5, 0, 0, 5, 10, 15, 15, 10, 5, 0, -5, 0, 5, 10, 10, 5, 0, -5, -10, -5, 0, 0, 0, 0,
    -5, -10, -20, -10, -5, -5, -5, -5, -10, -20,
];

pub(crate) const BISHOP_PST_EG: [i16; 64] = [
    -5, -5, -5, -5, -5, -5, -5, -5, -5, 10, 5, 5, 5, 5, 10, -5, -5, 5, 10, 10, 10, 10, 5, -5, -5,
    5, 10, 10, 10, 10, 5, -5, -5, 5, 10, 10, 10, 10, 5, -5, -5, 5, 5, 10, 10, 5, 5, -5, -5, 10, 5,
    5, 5, 5, 10, -5, -5, -5, -5, -5, -5, -5, -5, -5,
];

pub(crate) const ROOK_PST_EG: [i16; 64] = [
   10, 10, 10, 10, 10, 10, 10, 10,
   15, 15, 15, 15, 15, 15, 15, 15,
    0,  0,  0,  0,  0,  0,  0,  0,
    0,  0,  0,  0,  0,  0,  0,  0,
    0,  0,  0,  0,  0,  0,  0,  0,
    0,  0,  0,  0,  0,  0,  0,  0,
    0,  0,  0,  0,  0,  0,  0,  0,
    0,  0,  0,  0,  0,  0,  0,  0,
];

pub(crate) const QUEEN_PST_EG: [i16; 64] = [
    -10, -5, -5, 0, 0, -5, -5, -10, -5, 0, 0, 0, 0, 0, 0, -5, -5, 0, 5, 5, 5, 5, 0, -5, 0, 0, 5,
    5, 5, 5, 0, 0, 0, 0, 5, 5, 5, 5, 0, 0, -5, 0, 0, 0, 0, 0, 0, -5, -5, 0, 0, 0, 0, 0, 0, -5,
    -10, -5, -5, 0, 0, -5, -5, -10,
];

pub(crate) const KING_PST_ENDGAME: [i16; 64] = [
    -50, -30, -30, -30, -30, -30, -30, -50, -30, -20, -10, 0, 0, -10, -20, -30, -30, -10, 20, 30,
    30, 20, -10, -30, -30, -10, 30, 40, 40, 30, -10, -30, -30, -10, 30, 40, 40, 30, -10, -30, -30,
    -10, 20, 30, 30, 20, -10, -30, -30, -20, -10, 0, 0, -10, -20, -30, -50, -30, -30, -30, -30,
    -30, -30, -50,
];

/// Get piece-square table value for a piece at a position (single-phase, MG only)
pub(crate) fn get_pst_value(piece: i8, square: i8) -> i16 {
    let abs_piece = piece.abs();
    let is_white = piece > 0;

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

/// Get tapered piece-square value (MG and EG interpolated by phase)
pub(crate) fn get_pst_value_tapered(piece: i8, square: i8, phase: i16) -> i16 {
    let abs_piece = piece.abs();
    let is_white = piece > 0;

    let pst_index = if is_white {
        square as usize
    } else {
        (63 - square) as usize
    };

    let (mg, eg) = match abs_piece {
        PAWN_ID => (PAWN_PST[pst_index], PAWN_PST_EG[pst_index]),
        KNIGHT_ID => (KNIGHT_PST[pst_index], KNIGHT_PST_EG[pst_index]),
        BISHOP_ID => (BISHOP_PST[pst_index], BISHOP_PST_EG[pst_index]),
        ROOK_ID => (ROOK_PST[pst_index], ROOK_PST_EG[pst_index]),
        QUEEN_ID => (QUEEN_PST[pst_index], QUEEN_PST_EG[pst_index]),
        KING_ID => (KING_PST_MIDDLEGAME[pst_index], KING_PST_ENDGAME[pst_index]),
        _ => (0, 0),
    };

    let mg_frac = phase as i16;
    let eg_frac = (MAX_PHASE - phase) as i16;
    let value = (mg * mg_frac + eg * eg_frac) / MAX_PHASE;

    if is_white { value } else { -value }
}
