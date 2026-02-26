//! Chess engine resource – board state management backed by shakmaty.
//!
//! `ChessEngine` is the Bevy ECS [`Resource`] that:
//! - Holds the authoritative board position as a FEN string
//! - Generates legal moves for any piece using `shakmaty`
//! - Validates moves (does not leave king in check)
//! - Can sync the ECS piece positions back to update the internal FEN
//!
//! The Stockfish subprocess (`stockfish-uci` crate) is used for AI move
//! generation; this resource handles move *validation* only.

use crate::game::components::HasMoved;
use crate::game::resources::turn::CurrentTurn;
use crate::rendering::pieces::{Piece, PieceColor, PieceType};
use bevy::prelude::*;
use shakmaty::{fen::Fen, CastlingMode, Chess, Color, Position, Square};

/// The starting position FEN.
const STARTING_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

/// Bevy Resource wrapping the board position.
///
/// Stores the position as a FEN string for easy serialisation and passing to
/// Stockfish. Legal move generation uses `shakmaty` in pure Rust.
#[derive(Resource)]
pub struct ChessEngine {
    /// Current position as a FEN string. Updated after every move.
    pub fen: String,
    /// Parsed shakmaty position (kept in sync with `fen`).
    position: Chess,
}

impl Default for ChessEngine {
    fn default() -> Self {
        let fen: Fen = STARTING_FEN.parse().expect("valid starting FEN");
        let position: Chess = fen
            .clone()
            .into_position(CastlingMode::Standard)
            .expect("valid starting position");
        Self {
            fen: STARTING_FEN.to_string(),
            position,
        }
    }
}

impl ChessEngine {
    // ─── Coordinate helpers ─────────────────────────────────────────────────

    /// Convert ECS `(x, y)` board coords to a UCI square string (e.g. `"e4"`).
    ///
    /// ECS coordinate system:
    /// - `x` = file index (0 = file a, 7 = file h)
    /// - `y` = rank index (0 = rank 1, 7 = rank 8)
    pub fn coords_to_uci(x: u8, y: u8) -> String {
        let file = (b'a' + x) as char;
        let rank = (b'1' + y) as char;
        format!("{}{}", file, rank)
    }

    /// Parse a UCI square string (e.g. `"e4"`) to ECS `(x, y)` coords.
    ///
    /// Returns `(file, rank)` where:
    /// - file = 0-7 (a-h)
    /// - rank = 0-7 (1-8)
    pub fn uci_to_coords(sq: &str) -> Option<(u8, u8)> {
        let chars: Vec<char> = sq.chars().collect();
        if chars.len() < 2 {
            return None;
        }
        let file = chars[0] as u8 - b'a';
        let rank = chars[1] as u8 - b'1';
        if rank < 8 && file < 8 {
            Some((file, rank))
        } else {
            None
        }
    }

    /// Convert file and rank to engine index (0-63).
    ///
    /// Index layout: rank * 8 + file (row-major order)
    #[inline]
    pub fn square_to_index(file: u8, rank: u8) -> i8 {
        (rank * 8 + file) as i8
    }

    #[inline]
    pub fn index_to_square(index: i8) -> (u8, u8) {
        let idx = index as u8;
        (idx % 8, idx / 8) // (file, rank)
    }

    // ─── Piece helpers ───────────────────────────────────────────────────────

    pub fn piece_color_to_engine(color: PieceColor) -> i64 {
        match color {
            PieceColor::White => 1,
            PieceColor::Black => -1,
        }
    }

    pub fn piece_type_to_id(piece_type: PieceType) -> i8 {
        match piece_type {
            PieceType::Pawn => 1,
            PieceType::Knight => 2,
            PieceType::Bishop => 3,
            PieceType::Rook => 4,
            PieceType::Queen => 5,
            PieceType::King => 6,
        }
    }

    // ─── Board sync ─────────────────────────────────────────────────────────

    /// Rebuild the internal FEN from ECS piece components.
    ///
    /// This is called before AI move generation to ensure Stockfish sees the
    /// latest board state.
    pub fn sync_ecs_to_engine(
        &mut self,
        pieces_query: &Query<(Entity, &Piece, &HasMoved)>,
        current_turn: &CurrentTurn,
    ) {
        self.sync_ecs_to_engine_impl(pieces_query.iter(), current_turn);
    }

    pub fn sync_ecs_to_engine_with_transform(
        &mut self,
        pieces_query: &Query<(Entity, &Piece, &HasMoved, &Transform)>,
        current_turn: &CurrentTurn,
    ) {
        self.sync_ecs_to_engine_impl(
            pieces_query.iter().map(|(e, p, h, _)| (e, p, h)),
            current_turn,
        );
    }

    pub fn sync_ecs_to_engine_impl<'a>(
        &mut self,
        pieces: impl Iterator<Item = (Entity, &'a Piece, &'a HasMoved)>,
        current_turn: &CurrentTurn,
    ) {
        // Build an 8×8 board array (same encoding as old chess_engine for compat)
        let mut board = [0i8; 64];
        let mut castling = CastlingRights::default();

        for (_, piece, has_moved) in pieces {
            let sq = Self::square_to_index(piece.x, piece.y) as usize;
            let id = Self::piece_type_to_id(piece.piece_type);
            board[sq] = if piece.color == PieceColor::White {
                id
            } else {
                -id
            };

            // Track castling rights
            if piece.piece_type == PieceType::King {
                if piece.color == PieceColor::White {
                    castling.white_king_moved = has_moved.moved;
                } else {
                    castling.black_king_moved = has_moved.moved;
                }
            } else if piece.piece_type == PieceType::Rook {
                match (piece.color, sq) {
                    (PieceColor::White, 0) => castling.wa_rook_moved = has_moved.moved,
                    (PieceColor::White, 7) => castling.wh_rook_moved = has_moved.moved,
                    (PieceColor::Black, 56) => castling.ba_rook_moved = has_moved.moved,
                    (PieceColor::Black, 63) => castling.bh_rook_moved = has_moved.moved,
                    _ => {}
                }
            }
        }

        self.fen = board_to_fen(&board, current_turn, &castling);
        self.refresh_position();
    }

    /// Re-parse internal `position` from `self.fen`.
    pub fn refresh_position(&mut self) {
        if let Ok(fen) = self.fen.parse::<Fen>() {
            if let Ok(pos) = fen.into_position(CastlingMode::Standard) {
                self.position = pos;
            }
        }
    }

    /// Update the FEN after a move (call after apply_uci_move succeeds).
    pub fn apply_fen(&mut self, new_fen: &str) {
        self.fen = new_fen.to_string();
        self.refresh_position();
    }

    /// Reset to the starting position.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    // ─── Move generation ────────────────────────────────────────────────────

    /// Return all legal destination squares for the piece at ECS coords `square`.
    pub fn get_legal_moves_for_square(
        &mut self,
        square: (u8, u8),
        color: PieceColor,
    ) -> Vec<(u8, u8)> {
        let src_uci = Self::coords_to_uci(square.0, square.1);
        let src_sq: Square = match src_uci.parse() {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let sm_color = match color {
            PieceColor::White => Color::White,
            PieceColor::Black => Color::Black,
        };

        if self.position.turn() != sm_color {
            return vec![];
        }

        self.position
            .legal_moves()
            .into_iter()
            .filter(|mv| mv.from() == Some(src_sq))
            .filter_map(|mv| {
                let to = mv.to();
                let uci = to.to_string(); // "e4" etc.
                Self::uci_to_coords(&uci)
            })
            .collect()
    }

    /// Get the current FEN string (for passing to Stockfish or the backend).
    pub fn current_fen(&self) -> &str {
        &self.fen
    }

    /// Check if the current side to move is in check.
    pub fn is_check(&self) -> bool {
        self.position.is_check()
    }

    /// Return all legal moves for the current position.
    pub fn legal_moves(&self) -> shakmaty::MoveList {
        self.position.legal_moves()
    }
}

// ─── Castling rights helper ──────────────────────────────────────────────────

#[derive(Default)]
struct CastlingRights {
    white_king_moved: bool,
    black_king_moved: bool,
    wa_rook_moved: bool,
    wh_rook_moved: bool,
    ba_rook_moved: bool,
    bh_rook_moved: bool,
}

/// Build a FEN string from a raw board array and current turn info.
fn board_to_fen(board: &[i8; 64], current_turn: &CurrentTurn, rights: &CastlingRights) -> String {
    let piece_char = |id: i8| -> char {
        let ch = match id.abs() {
            1 => 'p',
            2 => 'n',
            3 => 'b',
            4 => 'r',
            5 => 'q',
            6 => 'k',
            _ => '?',
        };
        if id > 0 {
            ch.to_ascii_uppercase()
        } else {
            ch
        }
    };

    // Ranks 8 → 1 (rank 7 index down to 0 in ECS x)
    let mut ranks = Vec::with_capacity(8);
    for rank in (0..8u8).rev() {
        let mut rank_str = String::new();
        let mut empty = 0u8;
        for file in 0..8u8 {
            let sq = (rank * 8 + file) as usize;
            let id = board[sq];
            if id == 0 {
                empty += 1;
            } else {
                if empty > 0 {
                    rank_str.push((b'0' + empty) as char);
                    empty = 0;
                }
                rank_str.push(piece_char(id));
            }
        }
        if empty > 0 {
            rank_str.push((b'0' + empty) as char);
        }
        ranks.push(rank_str);
    }

    let side = if current_turn.color == PieceColor::White {
        'w'
    } else {
        'b'
    };

    let mut castling_str = String::new();
    if !rights.white_king_moved && !rights.wh_rook_moved {
        castling_str.push('K');
    }
    if !rights.white_king_moved && !rights.wa_rook_moved {
        castling_str.push('Q');
    }
    if !rights.black_king_moved && !rights.bh_rook_moved {
        castling_str.push('k');
    }
    if !rights.black_king_moved && !rights.ba_rook_moved {
        castling_str.push('q');
    }
    if castling_str.is_empty() {
        castling_str.push('-');
    }

    format!(
        "{} {} {} - 0 {}",
        ranks.join("/"),
        side,
        castling_str,
        (current_turn.move_number + 1) / 2
    )
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coords_roundtrip() {
        // coords_to_uci(file, rank) where file=0..7 (a-h), rank=0..7 (1-8)
        assert_eq!(ChessEngine::coords_to_uci(0, 0), "a1");
        assert_eq!(ChessEngine::coords_to_uci(4, 0), "e1");
        assert_eq!(ChessEngine::coords_to_uci(4, 3), "e4");
        assert_eq!(ChessEngine::coords_to_uci(7, 7), "h8");
        // uci_to_coords returns (file, rank)
        assert_eq!(ChessEngine::uci_to_coords("e4"), Some((4, 3)));
        assert_eq!(ChessEngine::uci_to_coords("a1"), Some((0, 0)));
        assert_eq!(ChessEngine::uci_to_coords("h8"), Some((7, 7)));
    }

    #[test]
    fn default_engine_has_legal_moves() {
        let mut engine = ChessEngine::default();
        // White pawn on e2: file=e=4, rank=2-1=1 → (4, 1)
        let moves = engine.get_legal_moves_for_square((4, 1), PieceColor::White);
        assert!(
            !moves.is_empty(),
            "e2 pawn should have at least one legal move"
        );
    }

    #[test]
    fn test_square_to_index_legacy() {
        assert_eq!(ChessEngine::square_to_index(0, 0), 0);
        assert_eq!(ChessEngine::square_to_index(7, 7), 63);
    }

    #[test]
    fn test_board_to_fen_initial() {
        // Quick integration test checking if the start position builds correctly
        let mut board = [0i8; 64];
        // Set up White pieces on ranks 0 and 1
        board[0..8].copy_from_slice(&[4, 2, 3, 5, 6, 3, 2, 4]); // rank 0
        board[8..16].copy_from_slice(&[1; 8]); // rank 1
                                               // Set up Black pieces on ranks 6 and 7
        board[48..56].copy_from_slice(&[-1; 8]); // rank 6
        board[56..64].copy_from_slice(&[-4, -2, -3, -5, -6, -3, -2, -4]); // rank 7

        // Blank spaces from 16 to 48 are already 0
        let current_turn = CurrentTurn {
            color: PieceColor::White,
            move_number: 1,
        };
        let rights = CastlingRights::default(); // all false, meaning all castling allowed

        let fen = board_to_fen(&board, &current_turn, &rights);
        assert_eq!(
            fen,
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
        );
    }

    #[test]
    fn test_board_to_fen_moved_kings() {
        let mut board = [0i8; 64];
        board[0] = 6; // white king on a1
        board[63] = -6; // black king on h8

        let current_turn = CurrentTurn {
            color: PieceColor::Black,
            move_number: 6,
        };
        let rights = CastlingRights {
            white_king_moved: true,
            black_king_moved: true,
            ..Default::default()
        };

        let fen = board_to_fen(&board, &current_turn, &rights);
        // Both kings moved, no castling rights at all -> "-"
        // Turn is Black -> "b"
        // Move is 6 -> full move (6+1)/2 = 3
        assert_eq!(fen, "7k/8/8/8/8/8/8/K7 b - - 0 3");
    }
}
