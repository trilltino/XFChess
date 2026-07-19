//! Chess engine resource – board state management backed by nimzovich_engine.
//!
//! `ChessEngine` is the Bevy ECS [`Resource`] that:
//! - Holds the authoritative board position as a FEN string
//! - Generates legal moves for any piece using `nimzovich_engine`
//! - Validates moves (does not leave king in check)
//! - Can sync the ECS piece positions back to update the internal FEN

use crate::game::components::HasMoved;
use crate::rendering::pieces::{Piece, PieceColor, PieceType};
use bevy::prelude::*;
use nimzovich_engine::{
    game_from_fen, generate_pseudo_legal_moves, is_legal_move, set_game_from_fen, Game, BISHOP_ID,
    KING_ID, KNIGHT_ID, PAWN_ID, QUEEN_ID, ROOK_ID,
};
use std::collections::HashMap;

/// The starting position FEN.
const STARTING_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

/// Bevy Resource wrapping the board position.
#[derive(Resource)]
pub struct ChessEngine {
    /// Current position as a FEN string. Updated after every move.
    pub fen: String,
    /// Internal engine state.
    game: Game,
    /// Halfmove clock for 50-move rule.
    pub halfmove_clock: u32,
    /// Full move counter.
    pub fullmove_counter: u32,
    /// Current side to move.
    pub current_turn: PieceColor,
    /// Castling rights in KQkq notation.
    pub castling_rights: String,
    /// En passant target square in UCI notation.
    pub en_passant: Option<String>,
    /// Legal moves per source square, rebuilt once per turn after sync.
    /// Keyed by (file, rank). Empty map means cache is stale.
    move_cache: HashMap<(u8, u8), Vec<(u8, u8)>>,
    /// Set by execute_move after it syncs ECS→engine so update_game_phase can
    /// skip the redundant second sync and only rebuild the move cache.
    pub synced_this_move: bool,
    /// True once rebuild_legal_move_cache has run for the current turn.
    /// Prevents update_game_phase from re-syncing and re-building every frame
    /// when no move has occurred.
    pub move_cache_valid: bool,
}

/// A wrapper for a chess move to maintain some compatibility with the previous shakmaty-based API.
pub struct MoveWrapper {
    pub from: (u8, u8),
    pub to: (u8, u8),
}

impl MoveWrapper {
    pub fn from(&self) -> Option<UciSquare> {
        Some(UciSquare(self.from))
    }
    pub fn to(&self) -> UciSquare {
        UciSquare(self.to)
    }
}

pub struct UciSquare(pub (u8, u8));
impl ToString for UciSquare {
    fn to_string(&self) -> String {
        ChessEngine::coords_to_uci(self.0 .0, self.0 .1)
    }
}

impl Default for ChessEngine {
    fn default() -> Self {
        let game = game_from_fen(STARTING_FEN);
        Self {
            fen: STARTING_FEN.to_string(),
            game,
            halfmove_clock: 0,
            fullmove_counter: 1,
            current_turn: PieceColor::White,
            castling_rights: "KQkq".to_string(),
            en_passant: None,
            move_cache: HashMap::new(),
            synced_this_move: false,
            move_cache_valid: false,
        }
    }
}

impl ChessEngine {
    // ─── Coordinate helpers ─────────────────────────────────────────────────

    pub fn coords_to_uci(x: u8, y: u8) -> String {
        let file = (b'a' + x) as char;
        let rank = (b'1' + y) as char;
        format!("{}{}", file, rank)
    }

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

    #[inline]
    pub fn square_to_index(file: u8, rank: u8) -> i8 {
        (rank * 8 + file) as i8
    }

    #[inline]
    pub fn index_to_coords(index: i8) -> (u8, u8) {
        ((index % 8) as u8, (index / 8) as u8)
    }

    // ─── Piece helpers ───────────────────────────────────────────────────────

    pub fn piece_type_to_id(piece_type: PieceType) -> i8 {
        match piece_type {
            PieceType::Pawn => PAWN_ID,
            PieceType::Knight => KNIGHT_ID,
            PieceType::Bishop => BISHOP_ID,
            PieceType::Rook => ROOK_ID,
            PieceType::Queen => QUEEN_ID,
            PieceType::King => KING_ID,
        }
    }

    // ─── Board sync ─────────────────────────────────────────────────────────

    pub fn sync_ecs_to_engine_mut(
        &mut self,
        pieces_query: &mut Query<(Entity, &mut Piece, &mut HasMoved)>,
    ) {
        let pieces_data: Vec<(Entity, Piece, HasMoved)> = pieces_query
            .iter_mut()
            .map(|(e, p, h)| (e, *p, *h))
            .collect();

        self.sync_ecs_to_engine_impl(pieces_data.iter().map(|(e, p, h)| (*e, p, h)));
    }

    pub fn sync_ecs_to_engine(&mut self, pieces_query: &Query<(Entity, &Piece, &HasMoved)>) {
        self.sync_ecs_to_engine_impl(pieces_query.iter());
    }

    pub fn sync_ecs_to_engine_with_transform(
        &mut self,
        pieces_query: &Query<(Entity, &Piece, &HasMoved, &Transform)>,
    ) {
        self.sync_ecs_to_engine_impl(pieces_query.iter().map(|(e, p, h, _)| (e, p, h)));
    }

    pub fn sync_ecs_to_engine_impl<'a>(
        &mut self,
        pieces: impl Iterator<Item = (Entity, &'a Piece, &'a HasMoved)>,
    ) {
        self.move_cache.clear();
        self.move_cache_valid = false;
        let mut board = [0i8; 64];
        let mut castling = CastlingRights::default();

        for (_, piece, has_moved) in pieces {
            // Skip pieces that have been marked off-board (u8::MAX) — this happens
            // immediately before sync when a piece is captured, because FadingCapture
            // is applied via deferred Commands and the entity would otherwise appear
            // at the destination square alongside the capturing piece.
            if piece.x > 7 || piece.y > 7 {
                continue;
            }
            let sq = Self::square_to_index(piece.x, piece.y) as usize;
            let id = Self::piece_type_to_id(piece.piece_type);
            board[sq] = if piece.color == PieceColor::White {
                id
            } else {
                -id
            };

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

        let piece_placement = board_to_piece_placement(&board);
        let side = match self.current_turn {
            PieceColor::White => 'w',
            PieceColor::Black => 'b',
        };
        let castling_str = castling_to_string(&castling);
        let en_passant_str = self.en_passant.as_deref().unwrap_or("-");

        self.fen = format!(
            "{} {} {} {} {} {}",
            piece_placement,
            side,
            castling_str,
            en_passant_str,
            self.halfmove_clock,
            self.fullmove_counter
        );

        self.castling_rights = castling_str;
        self.refresh_position();
    }

    pub fn refresh_position(&mut self) {
        set_game_from_fen(&mut self.game, &self.fen);
    }

    // ─── Move generation ────────────────────────────────────────────────────

    /// Rebuild the legal-move cache for the current position.
    /// Call this once per turn after syncing the engine from ECS.
    /// All downstream per-click lookups read from this cache for free.
    pub fn rebuild_legal_move_cache(&mut self) {
        self.move_cache.clear();
        let side = if self.fen.contains(" w ") { 1 } else { -1 };
        let moves = generate_pseudo_legal_moves(&self.game, side);
        for mv in moves {
            if is_legal_move(&mut self.game, mv.src, mv.dst, side) {
                let from = Self::index_to_coords(mv.src);
                let to = Self::index_to_coords(mv.dst);
                self.move_cache.entry(from).or_default().push(to);
            }
        }
        self.move_cache_valid = true;
    }

    /// Returns cached legal destinations for a square. Returns empty if no cache entry.
    pub fn get_legal_moves_for_square(
        &self,
        square: (u8, u8),
        _color: PieceColor,
    ) -> Vec<(u8, u8)> {
        self.move_cache.get(&square).cloned().unwrap_or_default()
    }

    /// Check if a move expressed as a 4-char UCI string (e.g. "e2e4") is legal.
    /// Uses the cache when populated, otherwise falls back to a single legality test.
    pub fn is_move_legal_by_uci(&mut self, uci: &str) -> bool {
        if uci.len() < 4 {
            return false;
        }
        let from_str = &uci[0..2];
        let to_str = &uci[2..4];
        let Some(from) = Self::uci_to_coords(from_str) else {
            return false;
        };
        let Some(to) = Self::uci_to_coords(to_str) else {
            return false;
        };

        if !self.move_cache.is_empty() {
            return self
                .move_cache
                .get(&from)
                .map_or(false, |dsts| dsts.contains(&to));
        }

        // Fallback: single legality check without cache
        let side = if self.fen.contains(" w ") { 1 } else { -1 };
        let src = Self::square_to_index(from.0, from.1);
        let dst = Self::square_to_index(to.0, to.1);
        is_legal_move(&mut self.game, src, dst, side)
    }

    pub fn current_fen(&self) -> &str {
        &self.fen
    }

    pub fn is_check(&self) -> bool {
        let side = if self.fen.contains(" w ") { 1 } else { -1 };
        nimzovich_engine::is_in_check(&self.game, side)
    }

    pub fn has_legal_moves(&self) -> bool {
        !self.move_cache.is_empty()
    }

    /// SAN (Standard Algebraic Notation) for a move about to be applied,
    /// computed from the engine's *current* (pre-move) position — including
    /// correct disambiguation (e.g. `Nbd2` vs `Nfd2`) and promotion suffix.
    ///
    /// Must be called before the engine's internal position advances past
    /// this move (i.e. before `sync_ecs_to_engine*`/`refresh_position`).
    pub fn move_to_san(&self, from: (u8, u8), to: (u8, u8), promotion: Option<PieceType>) -> String {
        let src = Self::square_to_index(from.0, from.1);
        let dst = Self::square_to_index(to.0, to.1);
        let promo = promotion.map(Self::piece_type_to_id).unwrap_or(0);
        nimzovich_engine::move_to_san(&self.game, src, dst, promo)
    }

    pub fn legal_moves(&self) -> Vec<MoveWrapper> {
        self.move_cache
            .iter()
            .flat_map(|(&from, dsts)| dsts.iter().map(move |&to| MoveWrapper { from, to }))
            .collect()
    }

    // ─── FEN Import/Export ────────────────────────────────────────────────────

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

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn set_from_fen(&mut self, fen_str: &str) -> Result<(), String> {
        set_game_from_fen(&mut self.game, fen_str);
        self.fen = fen_str.to_string();

        let parts: Vec<&str> = fen_str.split_whitespace().collect();
        if parts.len() >= 6 {
            self.current_turn = if parts[1] == "w" {
                PieceColor::White
            } else {
                PieceColor::Black
            };
            self.castling_rights = parts[2].to_string();
            self.en_passant = if parts[3] == "-" {
                None
            } else {
                Some(parts[3].to_string())
            };
            self.halfmove_clock = parts[4].parse().unwrap_or(0);
            self.fullmove_counter = parts[5].parse().unwrap_or(1);
        }

        Ok(())
    }
}

#[derive(Default)]
struct CastlingRights {
    white_king_moved: bool,
    black_king_moved: bool,
    wa_rook_moved: bool,
    wh_rook_moved: bool,
    ba_rook_moved: bool,
    bh_rook_moved: bool,
}

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

fn board_to_piece_placement(board: &[i8; 64]) -> String {
    let piece_char = |id: i8| -> char {
        let ch = match id.abs() {
            PAWN_ID => 'p',
            KNIGHT_ID => 'n',
            BISHOP_ID => 'b',
            ROOK_ID => 'r',
            QUEEN_ID => 'q',
            KING_ID => 'k',
            _ => '?',
        };
        if id > 0 {
            ch.to_ascii_uppercase()
        } else {
            ch
        }
    };

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coords_roundtrip() {
        assert_eq!(ChessEngine::coords_to_uci(0, 0), "a1");
        assert_eq!(ChessEngine::coords_to_uci(4, 3), "e4");
        assert_eq!(ChessEngine::uci_to_coords("e4"), Some((4, 3)));
    }
}
