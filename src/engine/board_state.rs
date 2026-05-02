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
    /// Halfmove clock for 50-move rule (number of halfmoves since last capture or pawn move).
    pub halfmove_clock: u32,
    /// Full move counter (starts at 1, increments after Black's move).
    pub fullmove_counter: u32,
    /// Current side to move.
    pub current_turn: PieceColor,
    /// Castling rights in KQkq notation (e.g., "KQkq", "Kq", "-").
    pub castling_rights: String,
    /// En passant target square in UCI notation (e.g., "e3", "-" for none).
    pub en_passant: Option<String>,
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
            halfmove_clock: 0,
            fullmove_counter: 1,
            current_turn: PieceColor::White,
            castling_rights: "KQkq".to_string(),
            en_passant: None,
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

    // ─── Piece helpers ───────────────────────────────────────────────────────

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

    /// Rebuild the internal FEN from ECS piece components (mutable query version).
    ///
    /// This version accepts &mut Query which is what execute_move has.
    pub fn sync_ecs_to_engine_mut(
        &mut self,
        pieces_query: &mut Query<(Entity, &mut Piece, &mut HasMoved)>,
    ) {
        // Collect piece data to avoid a conflicting borrow on pieces_query.
        let pieces_data: Vec<(Entity, Piece, HasMoved)> = pieces_query
            .iter_mut()
            .map(|(e, p, h)| (e, *p, *h))
            .collect();

        self.sync_ecs_to_engine_impl(
            pieces_data.iter().map(|(e, p, h)| (*e, p, h)),
        );
    }

    /// Rebuild the internal FEN from ECS piece components.
    ///
    /// This is called before AI move generation to ensure Stockfish sees the
    /// latest board state.
    pub fn sync_ecs_to_engine(
        &mut self,
        pieces_query: &Query<(Entity, &Piece, &HasMoved)>,
    ) {
        self.sync_ecs_to_engine_impl(pieces_query.iter());
    }

    pub fn sync_ecs_to_engine_with_transform(
        &mut self,
        pieces_query: &Query<(Entity, &Piece, &HasMoved, &Transform)>,
    ) {
        self.sync_ecs_to_engine_impl(
            pieces_query.iter().map(|(e, p, h, _)| (e, p, h)),
        );
    }

    pub fn sync_ecs_to_engine_impl<'a>(
        &mut self,
        pieces: impl Iterator<Item = (Entity, &'a Piece, &'a HasMoved)>,
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

        // Build complete FEN string from board state
        let piece_placement = board_to_piece_placement(&board);
        // Use self.current_turn which was already updated by update_engine_state_after_move
        // The passed current_turn is the OLD turn before the move
        let side = match self.current_turn {
            PieceColor::White => 'w',
            PieceColor::Black => 'b',
        };
        let castling_str = castling_to_string(&castling);
        let en_passant_str = self.en_passant.as_deref().unwrap_or("-");

        // Complete FEN: piece_placement side castling en_passant halfmove fullmove
        self.fen = format!(
            "{} {} {} {} {} {}",
            piece_placement,
            side,
            castling_str,
            en_passant_str,
            self.halfmove_clock,
            self.fullmove_counter
        );

        // Only update castling_rights from ECS, NOT current_turn
        // current_turn was already set correctly by update_engine_state_after_move
        self.castling_rights = castling_str;
        // Note: halfmove_clock and en_passant are not updated from ECS sync
        // They should be updated during move execution

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

    // ─── FEN Import/Export ────────────────────────────────────────────────────

    /// Export the current board state to a FEN string.
    ///
    /// FEN format: `<piece_placement> <side_to_move> <castling_rights> <en_passant> <halfmove_clock> <fullmove_number>`
    ///
    /// Example: `rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1`
    pub fn to_fen(&self) -> String {
        let side = match self.current_turn {
            PieceColor::White => 'w',
            PieceColor::Black => 'b',
        };

        let en_passant_str = self.en_passant.as_deref().unwrap_or("-");

        format!(
            "{} {} {} {} {} {}",
            self.fen
                .split_whitespace()
                .next()
                .unwrap_or("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR"),
            side,
            self.castling_rights,
            en_passant_str,
            self.halfmove_clock,
            self.fullmove_counter
        )
    }

    /// Reset the engine to the starting position.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Set the engine state from a FEN string.
    pub fn set_from_fen(&mut self, fen_str: &str) -> Result<(), String> {
        let fen: Fen = fen_str.parse().map_err(|e| format!("Invalid FEN: {}", e))?;
        let position: Chess = fen
            .into_position(CastlingMode::Standard)
            .map_err(|e| format!("Invalid position from FEN: {}", e))?;
        
        // Update all fields
        self.fen = fen_str.to_string();
        self.position = position;
        
        // Extract auxiliary info if possible, or fallback to defaults
        let parts: Vec<&str> = fen_str.split_whitespace().collect();
        if parts.len() >= 6 {
            self.current_turn = if parts[1] == "w" { PieceColor::White } else { PieceColor::Black };
            self.castling_rights = parts[2].to_string();
            self.en_passant = if parts[3] == "-" { None } else { Some(parts[3].to_string()) };
            self.halfmove_clock = parts[4].parse().unwrap_or(0);
            self.fullmove_counter = parts[5].parse().unwrap_or(1);
        }
        
        Ok(())
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

/// Convert castling rights to KQkq notation string.
fn castling_to_string(rights: &CastlingRights) -> String {
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
    castling_str
}

/// Build the piece placement part of a FEN string from a raw board array.
fn board_to_piece_placement(board: &[i8; 64]) -> String {
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

    ranks.join("/")
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

}
