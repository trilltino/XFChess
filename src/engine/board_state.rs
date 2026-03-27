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

        // Build complete FEN string from board state
        let piece_placement = board_to_piece_placement(&board);
        let side = match current_turn.color {
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

        // Update state fields from ECS
        self.current_turn = current_turn.color;
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

    /// Update the FEN after a move (call after apply_uci_move succeeds).
    pub fn apply_fen(&mut self, new_fen: &str) {
        self.fen = new_fen.to_string();
        self.refresh_position();
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

    /// Load a board state from a FEN string.
    ///
    /// # Arguments
    /// * `fen` - A valid FEN string
    ///
    /// # Returns
    /// * `Ok(())` if the FEN was parsed successfully
    /// * `Err(String)` if the FEN is invalid
    ///
    /// # Example
    /// ```
    /// engine.from_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1")?;
    /// ```
    pub fn from_fen(&mut self, fen: &str) -> Result<(), String> {
        let fen_parsed: Fen = fen.parse().map_err(|e| format!("Invalid FEN: {:?}", e))?;
        let position: Chess = fen_parsed
            .clone()
            .into_position(CastlingMode::Standard)
            .map_err(|e| format!("Invalid position: {:?}", e))?;

        // Parse FEN fields
        let parts: Vec<&str> = fen.split_whitespace().collect();
        if parts.len() < 6 {
            return Err("FEN must have 6 fields".to_string());
        }

        // Side to move
        let current_turn = match parts[1] {
            "w" => PieceColor::White,
            "b" => PieceColor::Black,
            _ => return Err("Invalid side to move".to_string()),
        };

        // Castling rights
        let castling_rights = parts[2].to_string();

        // En passant
        let en_passant = if parts[3] == "-" {
            None
        } else {
            Some(parts[3].to_string())
        };

        // Halfmove clock
        let halfmove_clock: u32 = parts[4].parse().map_err(|_| "Invalid halfmove clock")?;

        // Fullmove counter
        let fullmove_counter: u32 = parts[5].parse().map_err(|_| "Invalid fullmove counter")?;

        self.fen = fen.to_string();
        self.position = position;
        self.current_turn = current_turn;
        self.castling_rights = castling_rights;
        self.en_passant = en_passant;
        self.halfmove_clock = halfmove_clock;
        self.fullmove_counter = fullmove_counter;

        Ok(())
    }

    /// Reset the engine to the starting position.
    pub fn reset(&mut self) {
        *self = Self::default();
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

/// Build a complete FEN string from a raw board array and current turn info.
fn board_to_fen(board: &[i8; 64], current_turn: &CurrentTurn, rights: &CastlingRights) -> String {
    let piece_placement = board_to_piece_placement(board);
    let side = if current_turn.color == PieceColor::White {
        'w'
    } else {
        'b'
    };
    let castling_str = castling_to_string(rights);

    format!(
        "{} {} {} - 0 {}",
        piece_placement,
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
