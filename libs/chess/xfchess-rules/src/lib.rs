//! Chess rules and notation shared by the game and server. No search engine or process.
use shakmaty::{
    fen::Fen, san::SanPlus, uci::UciMove, CastlingMode, Chess, EnPassantMode, Position,
};

mod pgn;
pub use pgn::*;

pub const STARTING_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
pub const PAWN_ID: i8 = 1;
pub const KNIGHT_ID: i8 = 2;
pub const BISHOP_ID: i8 = 3;
pub const ROOK_ID: i8 = 4;
pub const QUEEN_ID: i8 = 5;
pub const KING_ID: i8 = 6;

/// A small, cloneable rules position. Squares use a1=0 and h8=63.
#[derive(Clone, Debug)]
pub struct Game {
    position: Chess,
    pub board: [i8; 64],
}

impl Default for Game {
    fn default() -> Self {
        Self::from_position(Chess::default())
    }
}

impl Game {
    fn from_position(position: Chess) -> Self {
        let mut board = [0; 64];
        for (sq, piece) in position.board() {
            board[usize::from(sq)] =
                (piece.role as i8) * if piece.color.is_white() { 1 } else { -1 };
        }
        Self { position, board }
    }

    pub fn from_fen(fen: &str) -> Result<Self, String> {
        let fen: Fen = fen.parse().map_err(|e| format!("Invalid FEN: {e}"))?;
        let position = fen
            .into_position(CastlingMode::Standard)
            .map_err(|e| format!("Invalid position: {e}"))?;
        Ok(Self::from_position(position))
    }

    pub fn fen(&self) -> String {
        Fen::from_position(&self.position, EnPassantMode::Always).to_string()
    }
    pub fn white_to_move(&self) -> bool {
        self.position.turn().is_white()
    }
    pub fn is_check(&self) -> bool {
        self.position.is_check()
    }
    pub fn is_checkmate(&self) -> bool {
        self.position.is_checkmate()
    }
    pub fn is_stalemate(&self) -> bool {
        self.position.is_stalemate()
    }
    pub fn legal_moves(&self) -> Vec<String> {
        self.position
            .legal_moves()
            .iter()
            .map(|m| UciMove::from_move(*m, CastlingMode::Standard).to_string())
            .collect()
    }
    fn resolve_uci(&self, uci: &str) -> Result<shakmaty::Move, String> {
        if !matches!(uci.len(), 4 | 5) || !uci.is_ascii() {
            return Err(format!("Invalid UCI: {uci}"));
        }
        let parsed: UciMove = uci.parse().map_err(|e| format!("Invalid UCI: {e}"))?;
        let mv = parsed
            .to_move(&self.position)
            .map_err(|e| format!("Illegal move: {e}"))?;
        // The contract and the game use standard king-destination castling UCI.
        // Shakmaty also understands Chess960 king-to-rook notation; reject that alias.
        if UciMove::from_move(mv, CastlingMode::Standard).to_string() != uci {
            return Err(format!("Non-standard UCI: {uci}"));
        }
        Ok(mv)
    }
    pub fn is_legal_uci(&self, uci: &str) -> bool {
        self.resolve_uci(uci).is_ok()
    }
    pub fn san(&self, uci: &str) -> Result<String, String> {
        Ok(SanPlus::from_move(self.position.clone(), self.resolve_uci(uci)?).to_string())
    }
    pub fn uci_from_san(&self, san: &str) -> Result<String, String> {
        let san: SanPlus = san.parse().map_err(|e| format!("Invalid SAN: {e}"))?;
        let mv = san
            .san
            .to_move(&self.position)
            .map_err(|e| format!("Illegal SAN: {e}"))?;
        Ok(UciMove::from_move(mv, CastlingMode::Standard).to_string())
    }
    pub fn play_uci(&mut self, uci: &str) -> Result<(), String> {
        let mv = self.resolve_uci(uci)?;
        let mut position = self.position.clone();
        position.play_unchecked(mv);
        *self = Self::from_position(position);
        Ok(())
    }
    pub fn compact_board(&self) -> CompactBoard {
        let fen = self.fen();
        let parts: Vec<_> = fen.split_whitespace().collect();
        let castling = [('K', 1), ('Q', 2), ('k', 4), ('q', 8)]
            .iter()
            .fold(0, |flags, (c, bit)| {
                flags | if parts[2].contains(*c) { *bit } else { 0 }
            });
        let ep_target = if parts[3] == "-" {
            -1
        } else {
            ((parts[3].as_bytes()[1] - b'1') * 8 + parts[3].as_bytes()[0] - b'a') as i8
        };
        CompactBoard {
            squares: self.board,
            castling,
            ep_target,
            side_to_move: if self.white_to_move() { 1 } else { -1 },
        }
    }
}

/// The existing contract wire format. Clocks are intentionally absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompactBoard {
    pub squares: [i8; 64],
    pub castling: u8,
    pub ep_target: i8,
    pub side_to_move: i8,
}
impl CompactBoard {
    pub fn to_bytes(self) -> [u8; 68] {
        let mut out = [0; 68];
        for (dst, src) in out[..64].iter_mut().zip(self.squares) {
            *dst = src as u8;
        }
        out[64] = self.castling;
        out[65] = self.ep_target as u8;
        out[66] = self.side_to_move as u8;
        out
    }
    /// Preserve the contract's legacy FEN convention (no move clocks).
    pub fn to_fen(&self) -> String {
        let mut fen = String::new();
        for rank in (0..8).rev() {
            let mut empty = 0;
            for file in 0..8 {
                let p = self.squares[rank * 8 + file];
                if p == 0 {
                    empty += 1;
                    continue;
                }
                if empty > 0 {
                    fen.push(char::from(b'0' + empty));
                    empty = 0;
                }
                let c = b" PNBRQK"[p.unsigned_abs() as usize] as char;
                fen.push(if p > 0 { c } else { c.to_ascii_lowercase() });
            }
            if empty > 0 {
                fen.push(char::from(b'0' + empty));
            }
            if rank > 0 {
                fen.push('/');
            }
        }
        fen.push_str(if self.side_to_move > 0 { " w " } else { " b " });
        for (c, bit) in [('K', 1), ('Q', 2), ('k', 4), ('q', 8)] {
            if self.castling & bit != 0 {
                fen.push(c);
            }
        }
        if self.castling == 0 {
            fen.push('-');
        }
        fen.push(' ');
        if self.ep_target < 0 {
            fen.push('-');
        } else {
            fen.push((b'a' + self.ep_target as u8 % 8) as char);
            fen.push((b'1' + self.ep_target as u8 / 8) as char);
        }
        fen.push_str(" 0 1");
        fen
    }
}

pub fn new_game() -> Game {
    Game::default()
}
pub fn game_from_fen(fen: &str) -> Result<Game, String> {
    Game::from_fen(fen)
}
pub fn game_to_fen(game: &Game) -> String {
    game.fen()
}
pub fn move_uci(src: i8, dst: i8, promo: i8) -> Result<String, String> {
    if !(0..64).contains(&src) || !(0..64).contains(&dst) {
        return Err("Invalid square".into());
    }
    let suffix = match promo {
        0 => "",
        2 => "n",
        3 => "b",
        4 => "r",
        5 => "q",
        _ => return Err("Invalid promotion".into()),
    };
    Ok(format!(
        "{}{}{}{}{suffix}",
        (b'a' + src as u8 % 8) as char,
        (b'1' + src as u8 / 8) as char,
        (b'a' + dst as u8 % 8) as char,
        (b'1' + dst as u8 / 8) as char
    ))
}
pub fn parse_uci(bytes: &[u8; 5]) -> Result<(i8, i8, i8), String> {
    let raw = std::str::from_utf8(bytes)
        .map_err(|e| e.to_string())?
        .trim_end();
    let uci: UciMove = raw.parse().map_err(|e| format!("Invalid UCI: {e}"))?;
    match uci {
        UciMove::Normal {
            from,
            to,
            promotion,
        } => Ok((
            i8::from(from),
            i8::from(to),
            promotion.map_or(0, |r| r as i8),
        )),
        _ => Err("Expected a standard chess move".into()),
    }
}
pub fn san_to_move(game: &Game, san: &str) -> Result<(i8, i8, i8), String> {
    let uci = game.uci_from_san(san)?;
    let mut bytes = [b' '; 5];
    bytes[..uci.len()].copy_from_slice(uci.as_bytes());
    parse_uci(&bytes)
}
pub fn move_to_san(game: &Game, src: i8, dst: i8, promo: i8) -> Result<String, String> {
    game.san(&move_uci(src, dst, promo)?)
}
pub fn do_move_with_promo(game: &mut Game, src: i8, dst: i8, promo: i8) -> Result<(), String> {
    game.play_uci(&move_uci(src, dst, promo)?)
}
