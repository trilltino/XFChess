use crate::game::components::HasMoved;
use crate::rendering::pieces::{Piece, PieceColor, PieceType};
use bevy::prelude::*;
use std::collections::HashMap;
use xfchess_rules::{Game, BISHOP_ID, KING_ID, KNIGHT_ID, PAWN_ID, QUEEN_ID, ROOK_ID};

const STARTING_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

#[derive(Resource)]
pub struct ChessEngine {
    pub fen: String,
    game: Game,
    position_valid: bool,
    pub halfmove_clock: u32,
    pub fullmove_counter: u32,
    pub current_turn: PieceColor,
    pub castling_rights: String,
    pub en_passant: Option<String>,
    move_cache: HashMap<(u8, u8), Vec<(u8, u8)>>,
    pub synced_this_move: bool,
    pub move_cache_valid: bool,
}

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
impl std::fmt::Display for UciSquare {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&ChessEngine::coords_to_uci(self.0 .0, self.0 .1))
    }
}

impl Default for ChessEngine {
    fn default() -> Self {
        // Rules positions contain no search state.
        let game = Game::default();
        Self {
            fen: STARTING_FEN.to_string(),
            game,
            position_valid: true,
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
    pub fn coords_to_uci(x: u8, y: u8) -> String {
        let file = (b'a' + x) as char;
        let rank = (b'1' + y) as char;
        format!("{}{}", file, rank)
    }

    /// Board coordinates of a square such as `e4`; `None` for anything else.
    pub fn uci_to_coords(sq: &str) -> Option<(u8, u8)> {
        let &[file, rank, ..] = sq.as_bytes() else {
            return None;
        };
        let file = file.checked_sub(b'a').filter(|f| *f < 8)?;
        let rank = rank.checked_sub(b'1').filter(|r| *r < 8)?;
        Some((file, rank))
    }

    /// Splits a UCI move such as `e7e8q` into its squares and promotion piece.
    pub fn parse_uci_move(uci: &str) -> Option<((u8, u8), (u8, u8), Option<char>)> {
        if !matches!(uci.len(), 4 | 5) || !uci.is_ascii() {
            return None;
        }
        let from = Self::uci_to_coords(uci.get(0..2)?)?;
        let to = Self::uci_to_coords(uci.get(2..4)?)?;
        let promotion = if uci.len() == 5 {
            let c = uci.as_bytes()[4] as char;
            if !"qrbn".contains(c) {
                return None;
            }
            Some(c)
        } else {
            None
        };
        Some((from, to, promotion))
    }

    #[inline]
    pub fn square_to_index(file: u8, rank: u8) -> i8 {
        (rank * 8 + file) as i8
    }

    #[inline]
    pub fn index_to_coords(index: i8) -> (u8, u8) {
        ((index % 8) as u8, (index / 8) as u8)
    }

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
        let mut castling = CastlingRights::all_moved();

        for (_, piece, has_moved) in pieces {
            // Skip off-board captures: deferred FadingCapture commands have not run yet,
            // so the captured entity can still share the destination square.
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
        self.move_cache.clear();
        self.move_cache_valid = false;
        match Game::from_fen(&self.fen) {
            Ok(game) => {
                self.game = game;
                self.position_valid = true;
            }
            Err(error) => {
                self.position_valid = false;
                warn!("Invalid board position: {error}");
                self.move_cache.clear();
                self.move_cache_valid = false;
            }
        }
    }

    pub fn rebuild_legal_move_cache(&mut self) {
        self.move_cache.clear();
        if !self.position_valid {
            self.move_cache_valid = false;
            return;
        }
        for uci in self.game.legal_moves() {
            if let Some((from, to, _)) = Self::parse_uci_move(&uci) {
                let destinations = self.move_cache.entry(from).or_default();
                if !destinations.contains(&to) {
                    destinations.push(to);
                }
            }
        }
        self.move_cache_valid = true;
    }

    pub fn get_legal_moves_for_square(
        &self,
        square: (u8, u8),
        _color: PieceColor,
    ) -> Vec<(u8, u8)> {
        self.move_cache.get(&square).cloned().unwrap_or_default()
    }

    pub fn is_move_legal_by_uci(&mut self, uci: &str) -> bool {
        self.position_valid && self.game.is_legal_uci(uci)
    }

    pub fn current_fen(&self) -> &str {
        &self.fen
    }

    pub fn side_to_move(&self) -> PieceColor {
        if self.fen.contains(" w ") {
            PieceColor::White
        } else {
            PieceColor::Black
        }
    }

    pub fn is_check(&self) -> bool {
        self.position_valid && self.game.is_check()
    }

    pub fn has_legal_moves(&self) -> bool {
        !self.move_cache.is_empty()
    }

    /// The contract's insufficient-material draw (see `xfchess_rules`).
    pub fn is_insufficient_material(&self) -> bool {
        self.position_valid && self.game.is_insufficient_material()
    }

    /// The contract ends the game as a draw after 100 halfmoves without a
    /// capture or pawn move.
    pub fn fifty_move_rule_reached(&self) -> bool {
        self.position_valid && self.game.halfmove_clock() >= xfchess_rules::FIFTY_MOVE_HALFMOVES
    }

    pub fn move_to_san(
        &mut self,
        from: (u8, u8),
        to: (u8, u8),
        promotion: Option<PieceType>,
    ) -> String {
        let src = Self::square_to_index(from.0, from.1);
        let dst = Self::square_to_index(to.0, to.1);
        let promo = promotion.map(Self::piece_type_to_id).unwrap_or(0);
        xfchess_rules::move_to_san(&self.game, src, dst, promo).unwrap_or_else(|e| {
            warn!("SAN: {e}");
            String::new()
        })
    }

    pub fn legal_moves(&self) -> Vec<MoveWrapper> {
        self.move_cache
            .iter()
            .flat_map(|(&from, dsts)| dsts.iter().map(move |&to| MoveWrapper { from, to }))
            .collect()
    }

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
        self.game = Game::from_fen(fen_str)?;
        self.position_valid = true;
        self.move_cache.clear();
        self.move_cache_valid = false;
        self.fen = self.game.fen();

        let parts: Vec<&str> = self.fen.split_whitespace().collect();
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

impl CastlingRights {
    fn all_moved() -> Self {
        Self {
            white_king_moved: true,
            black_king_moved: true,
            wa_rook_moved: true,
            wh_rook_moved: true,
            ba_rook_moved: true,
            bh_rook_moved: true,
        }
    }
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
        let ch = match id.checked_abs() {
            Some(PAWN_ID) => 'p',
            Some(KNIGHT_ID) => 'n',
            Some(BISHOP_ID) => 'b',
            Some(ROOK_ID) => 'r',
            Some(QUEEN_ID) => 'q',
            Some(KING_ID) => 'k',
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

    #[test]
    fn malformed_squares_are_rejected_without_panicking() {
        // Squares arrive from the network; none of these may underflow.
        for bad in ["", "e", "5a", "a0", "i1", "a9", "\u{e9}1", "A1"] {
            assert_eq!(ChessEngine::uci_to_coords(bad), None, "{bad:?}");
        }
        assert_eq!(ChessEngine::uci_to_coords("h8q"), Some((7, 7)));
    }

    const FOOLS_MATE_FEN: &str = "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3";

    #[test]
    fn side_to_move_matches_fen_field() -> Result<(), Box<dyn std::error::Error>> {
        let mut engine = ChessEngine::default();
        assert_eq!(
            engine.side_to_move(),
            PieceColor::White,
            "start pos is white"
        );

        engine.set_from_fen(FOOLS_MATE_FEN)?;
        assert_eq!(
            engine.side_to_move(),
            PieceColor::White,
            "FEN says 'w' — white is to move and is the mated side"
        );

        engine.set_from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR b KQkq - 0 1")?;
        assert_eq!(engine.side_to_move(), PieceColor::Black);

        Ok(())
    }

    #[test]
    fn fools_mate_attributes_checkmate_to_white() -> Result<(), Box<dyn std::error::Error>> {
        let mut engine = ChessEngine::default();
        engine.set_from_fen(FOOLS_MATE_FEN)?;
        engine.rebuild_legal_move_cache();

        assert!(engine.is_check(), "white king must be in check");
        assert!(
            !engine.has_legal_moves(),
            "white must have no legal escape — this is mate"
        );

        // Mirrors `update_game_phase`'s branch exactly.
        let loser = engine.side_to_move();
        assert_eq!(loser, PieceColor::White, "white is checkmated, not black");

        Ok(())
    }

    #[test]
    fn black_mated_position_attributes_to_black() -> Result<(), Box<dyn std::error::Error>> {
        let mut engine = ChessEngine::default();
        // 1.e4 e5 2.Bc4 Nc6 3.Qh5 Nf6?? 4.Qxf7# — black to move, black mated.
        engine
            .set_from_fen("r1bqkb1r/pppp1Qpp/2n2n2/4p3/2B1P3/8/PPPP1PPP/RNB1K1NR b KQkq - 0 4")?;
        engine.rebuild_legal_move_cache();

        assert!(engine.is_check(), "black king must be in check");
        assert!(!engine.has_legal_moves(), "black must be mated");
        assert_eq!(engine.side_to_move(), PieceColor::Black);

        Ok(())
    }
}
