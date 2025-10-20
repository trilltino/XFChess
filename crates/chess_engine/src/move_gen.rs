//! Move generation with precalculated move tables
//!
//! This module generates all legal moves for a position using precalculated
//! move tables for efficiency. Tables are computed once at startup.

use super::types::*;
use super::constants::*;
use super::board::*;

/// Initialize move tables for all piece types
pub fn init_move_tables(game: &mut Game) {
    // Initialize sliding pieces
    for square in 0..64 {
        game.rook[square] = init_rook_moves_from(square as i8);
        game.bishop[square] = init_bishop_moves_from(square as i8);
        game.knight[square] = init_knight_moves_from(square as i8);
        game.king[square] = init_king_moves_from(square as i8);
        game.white_pawn[square] = init_white_pawn_moves_from(square as i8);
        game.black_pawn[square] = init_black_pawn_moves_from(square as i8);
    }
}

/// Generate rook moves from a square
fn init_rook_moves_from(from: i8) -> KKS {
    let mut moves = Vec::new();
    let (col, row) = pos_to_square(from);

    for &dir in &ROOK_DIRS {
        let mut current = from as i32;
        loop {
            current += dir;

            if !is_valid_pos(current) {
                break;
            }

            let (new_col, new_row) = pos_to_square(current as i8);

            // Check if we wrapped around the board
            if (dir == O && new_col < col as i8) ||
               (dir == W && new_col > col as i8) ||
               (dir == N && new_row > row as i8) ||
               (dir == S && new_row < row as i8) {
                break;
            }

            moves.push(KK::new(from, current as i8, 0, 0));
        }
    }

    moves
}

/// Generate bishop moves from a square
fn init_bishop_moves_from(from: i8) -> KKS {
    let mut moves = Vec::new();
    let (col, row) = pos_to_square(from);

    for &dir in &BISHOP_DIRS {
        let mut current = from as i32;
        loop {
            current += dir;

            if !is_valid_pos(current) {
                break;
            }

            let (new_col, new_row) = pos_to_square(current as i8);

            // Check bounds and wrapping
            if new_col < 0 || new_col > 7 || new_row < 0 || new_row > 7 {
                break;
            }

            // Check if we wrapped around
            if (new_col - col as i8).abs() > 1 && (new_row - row as i8).abs() != (current as i8 - from).abs() / 8 {
                break;
            }

            moves.push(KK::new(from, current as i8, 0, 0));
        }
    }

    moves
}

/// Generate knight moves from a square
fn init_knight_moves_from(from: i8) -> KKS {
    let mut moves = Vec::new();
    let (col, row) = pos_to_square(from);

    for &dir in &KNIGHT_DIRS {
        let to = from as i32 + dir;

        if !is_valid_pos(to) {
            continue;
        }

        let (new_col, new_row) = pos_to_square(to as i8);

        // Knight moves are exactly 2+1 or 1+2 squares
        let col_diff = (new_col - col as i8).abs();
        let row_diff = (new_row - row as i8).abs();

        if (col_diff == 2 && row_diff == 1) || (col_diff == 1 && row_diff == 2) {
            moves.push(KK::new(from, to as i8, 0, 0));
        }
    }

    moves
}

/// Generate king moves from a square
fn init_king_moves_from(from: i8) -> KKS {
    let mut moves = Vec::new();
    let (col, row) = pos_to_square(from);

    for &dir in &KING_DIRS {
        let to = from as i32 + dir;

        if !is_valid_pos(to) {
            continue;
        }

        let (new_col, new_row) = pos_to_square(to as i8);

        // King moves exactly 1 square
        if (new_col - col as i8).abs() <= 1 && (new_row - row as i8).abs() <= 1 {
            moves.push(KK::new(from, to as i8, 0, 0));
        }
    }

    moves
}

/// Generate white pawn moves from a square
fn init_white_pawn_moves_from(from: i8) -> KKS {
    let mut moves = Vec::new();
    let (col, row) = pos_to_square(from);

    // Single push forward
    let to = from - 8;
    if to >= 0 {
        moves.push(KK::new(from, to, 0, 0));
    }

    // Double push from starting position
    if row == 1 {
        let to = from - 16;
        if to >= 0 {
            moves.push(KK::new(from, to, 0, 0));
        }
    }

    // Captures (diagonal)
    for &dir in &[NO, NW] {
        let to = from as i32 + dir;
        if is_valid_pos(to) {
            let (new_col, _) = pos_to_square(to as i8);
            if (new_col - col as i8).abs() == 1 {
                moves.push(KK::new(from, to as i8, 0, 0));
            }
        }
    }

    moves
}

/// Generate black pawn moves from a square
fn init_black_pawn_moves_from(from: i8) -> KKS {
    let mut moves = Vec::new();
    let (col, row) = pos_to_square(from);

    // Single push forward
    let to = from + 8;
    if to < 64 {
        moves.push(KK::new(from, to, 0, 0));
    }

    // Double push from starting position
    if row == 6 {
        let to = from + 16;
        if to < 64 {
            moves.push(KK::new(from, to, 0, 0));
        }
    }

    // Captures (diagonal)
    for &dir in &[SO, SW] {
        let to = from as i32 + dir;
        if is_valid_pos(to) {
            let (new_col, _) = pos_to_square(to as i8);
            if (new_col - col as i8).abs() == 1 {
                moves.push(KK::new(from, to as i8, 0, 0));
            }
        }
    }

    moves
}

/// Generate all pseudo-legal moves for a color
pub fn generate_pseudo_legal_moves(game: &Game, color: Color) -> Vec<KK> {
    let mut moves = Vec::with_capacity(200);

    for square in 0..64 {
        let piece = game.board[square];

        if !piece_belongs_to(piece, color) {
            continue;
        }

        let piece_type = piece.abs();

        match piece_type {
            PAWN_ID => generate_pawn_moves(game, square as i8, color, &mut moves),
            KNIGHT_ID => generate_knight_moves(game, square as i8, color, &mut moves),
            BISHOP_ID => generate_bishop_moves(game, square as i8, color, &mut moves),
            ROOK_ID => generate_rook_moves(game, square as i8, color, &mut moves),
            QUEEN_ID => generate_queen_moves(game, square as i8, color, &mut moves),
            KING_ID => generate_king_moves(game, square as i8, color, &mut moves),
            _ => {}
        }
    }

    moves
}

fn generate_pawn_moves(game: &Game, from: i8, color: Color, moves: &mut Vec<KK>) {
    let candidates = if color > 0 {
        &game.white_pawn[from as usize]
    } else {
        &game.black_pawn[from as usize]
    };

    let (from_col, _from_row) = pos_to_square(from);
    let forward_dir = if color > 0 { -8 } else { 8 };

    for candidate in candidates {
        let to = candidate.dst;
        let (to_col, _to_row) = pos_to_square(to);
        let dest_piece = game.board[to as usize];

        // Check if it's a capture or push
        let is_diagonal = (to_col - from_col).abs() == 1;

        if is_diagonal {
            // Diagonal moves are only valid for captures
            if dest_piece != 0 && !piece_belongs_to(dest_piece, color) {
                moves.push(*candidate);
            }
        } else {
            // Forward moves must be to empty square
            if dest_piece == 0 {
                // For double push, check intermediate square
                if (to - from).abs() == 16 {
                    let intermediate = (from as i32 + forward_dir) as i8;
                    if game.board[intermediate as usize] == 0 {
                        moves.push(*candidate);
                    }
                } else {
                    moves.push(*candidate);
                }
            }
        }
    }
}

fn generate_knight_moves(game: &Game, from: i8, color: Color, moves: &mut Vec<KK>) {
    for candidate in &game.knight[from as usize] {
        let dest_piece = game.board[candidate.dst as usize];
        if dest_piece == 0 || !piece_belongs_to(dest_piece, color) {
            moves.push(*candidate);
        }
    }
}

fn generate_sliding_moves(game: &Game, from: i8, color: Color, table: &[KK], moves: &mut Vec<KK>) {
    let mut blocked_dirs = [false; 8];

    for candidate in table {
        let dir_idx = ((candidate.dst - from) / ((candidate.dst - from).abs().max(1))) as usize;

        if blocked_dirs.get(dir_idx % 8).copied().unwrap_or(false) {
            continue;
        }

        let dest_piece = game.board[candidate.dst as usize];

        if dest_piece == 0 {
            moves.push(*candidate);
        } else if !piece_belongs_to(dest_piece, color) {
            moves.push(*candidate);
            blocked_dirs[dir_idx % 8] = true;
        } else {
            blocked_dirs[dir_idx % 8] = true;
        }
    }
}

fn generate_bishop_moves(game: &Game, from: i8, color: Color, moves: &mut Vec<KK>) {
    generate_sliding_moves(game, from, color, &game.bishop[from as usize], moves);
}

fn generate_rook_moves(game: &Game, from: i8, color: Color, moves: &mut Vec<KK>) {
    generate_sliding_moves(game, from, color, &game.rook[from as usize], moves);
}

fn generate_queen_moves(game: &Game, from: i8, color: Color, moves: &mut Vec<KK>) {
    generate_bishop_moves(game, from, color, moves);
    generate_rook_moves(game, from, color, moves);
}

fn generate_king_moves(game: &Game, from: i8, color: Color, moves: &mut Vec<KK>) {
    for candidate in &game.king[from as usize] {
        let dest_piece = game.board[candidate.dst as usize];
        if dest_piece == 0 || !piece_belongs_to(dest_piece, color) {
            moves.push(*candidate);
        }
    }
}

/// Check if a square is under attack by the opponent
pub fn is_square_attacked(game: &Game, square: i8, by_color: Color) -> bool {
    // Check all opponent pieces to see if they can attack this square
    for from in 0..64 {
        let piece = game.board[from];

        if !piece_belongs_to(piece, by_color) {
            continue;
        }

        let piece_type = piece.abs();
        let can_attack = match piece_type {
            PAWN_ID => can_pawn_attack(game, from as i8, square, by_color),
            KNIGHT_ID => can_knight_attack(game, from as i8, square),
            BISHOP_ID => can_bishop_attack(game, from as i8, square),
            ROOK_ID => can_rook_attack(game, from as i8, square),
            QUEEN_ID => can_queen_attack(game, from as i8, square),
            KING_ID => can_king_attack(game, from as i8, square),
            _ => false,
        };

        if can_attack {
            return true;
        }
    }

    false
}

fn can_pawn_attack(game: &Game, from: i8, to: i8, color: Color) -> bool {
    let candidates = if color > 0 {
        &game.white_pawn[from as usize]
    } else {
        &game.black_pawn[from as usize]
    };

    let (from_col, _) = pos_to_square(from);
    let (to_col, _) = pos_to_square(to);

    candidates.iter().any(|m| m.dst == to && (to_col - from_col).abs() == 1)
}

fn can_knight_attack(_game: &Game, from: i8, to: i8) -> bool {
    let (from_col, from_row) = pos_to_square(from);
    let (to_col, to_row) = pos_to_square(to);
    let col_diff = (to_col - from_col).abs();
    let row_diff = (to_row - from_row).abs();
    (col_diff == 2 && row_diff == 1) || (col_diff == 1 && row_diff == 2)
}

fn can_bishop_attack(game: &Game, from: i8, to: i8) -> bool {
    let (from_col, from_row) = pos_to_square(from);
    let (to_col, to_row) = pos_to_square(to);

    if (to_col - from_col).abs() != (to_row - from_row).abs() {
        return false;
    }

    let col_dir = (to_col - from_col).signum();
    let row_dir = (to_row - from_row).signum();

    let mut current_col = from_col + col_dir;
    let mut current_row = from_row + row_dir;

    while current_col != to_col {
        let pos = square_to_pos(current_col, current_row);
        if game.board[pos as usize] != 0 {
            return false;
        }
        current_col += col_dir;
        current_row += row_dir;
    }

    true
}

fn can_rook_attack(game: &Game, from: i8, to: i8) -> bool {
    let (from_col, from_row) = pos_to_square(from);
    let (to_col, to_row) = pos_to_square(to);

    if from_col != to_col && from_row != to_row {
        return false;
    }

    if from_col == to_col {
        let dir = (to_row - from_row).signum();
        let mut current = from_row + dir;
        while current != to_row {
            let pos = square_to_pos(from_col, current);
            if game.board[pos as usize] != 0 {
                return false;
            }
            current += dir;
        }
    } else {
        let dir = (to_col - from_col).signum();
        let mut current = from_col + dir;
        while current != to_col {
            let pos = square_to_pos(current, from_row);
            if game.board[pos as usize] != 0 {
                return false;
            }
            current += dir;
        }
    }

    true
}

fn can_queen_attack(game: &Game, from: i8, to: i8) -> bool {
    can_rook_attack(game, from, to) || can_bishop_attack(game, from, to)
}

fn can_king_attack(_game: &Game, from: i8, to: i8) -> bool {
    let (from_col, from_row) = pos_to_square(from);
    let (to_col, to_row) = pos_to_square(to);
    (to_col - from_col).abs() <= 1 && (to_row - from_row).abs() <= 1
}

/// Find the king position for a color
pub fn find_king(game: &Game, color: Color) -> Option<i8> {
    let king_piece = if color > 0 { W_KING } else { B_KING };

    for square in 0..64 {
        if game.board[square] == king_piece {
            return Some(square as i8);
        }
    }

    None
}

/// Check if the king of a color is in check
pub fn is_in_check(game: &Game, color: Color) -> bool {
    if let Some(king_pos) = find_king(game, color) {
        is_square_attacked(game, king_pos, -color)
    } else {
        false
    }
}
