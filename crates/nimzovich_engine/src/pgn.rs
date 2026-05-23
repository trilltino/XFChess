//! PGN (Portable Game Notation) generation
//!
//! Provides SAN move generation and PGN document assembly.
//! Gated behind the `std` feature.

use std::collections::BTreeMap;
use std::string::String;
use std::vec::Vec;

use crate::api::is_legal_move;
use crate::constants::*;
use crate::move_gen::*;
use crate::types::*;

// ---------------------------------------------------------------------------
// SAN generation
// ---------------------------------------------------------------------------

/// Convert a move to Standard Algebraic Notation (SAN).
///
/// # Arguments
/// * `game` - Game state *before* the move is applied
/// * `src` - Source square (0-63)
/// * `dst` - Destination square (0-63)
/// * `promo` - Promotion piece ID (0 = none, 2-5 = N/B/R/Q)
///
/// # Returns
/// SAN string (e.g., "Nxf7+", "e8=Q", "O-O-O")
pub fn move_to_san(game: &Game, src: i8, dst: i8, promo: i8) -> String {
    let piece = game.board[src as usize];
    let piece_type = piece.abs() as usize;
    let color = if piece > 0 { COLOR_WHITE } else { COLOR_BLACK };

    // Castling
    if piece_type == KING_ID as usize && (dst - src).abs() == 2 {
        if dst > src {
            return "O-O".to_string();
        } else {
            return "O-O-O".to_string();
        }
    }

    // Generate all pseudo-legal moves of the same piece type to the same dst
    let all_moves = generate_pseudo_legal_moves(game, color);
    let same_piece_same_dst: Vec<&KK> = all_moves
        .iter()
        .filter(|m| {
            m.dst == dst
                && game.board[m.src as usize].abs() == piece_type as i8
                && m.src != src
        })
        .collect();

    // Disambiguation
    let mut disambiguation = String::new();
    if !same_piece_same_dst.is_empty() {
        let src_file = src % 8;
        let src_rank = src / 8;

        let same_file = same_piece_same_dst.iter().any(|m| m.src % 8 == src_file);
        let same_rank = same_piece_same_dst.iter().any(|m| m.src / 8 == src_rank);

        if same_file && same_rank {
            // Both file and rank needed
            disambiguation.push(file_char(src_file));
            disambiguation.push(rank_char(src_rank));
        } else if same_file {
            // Same file — disambiguate by rank
            disambiguation.push(rank_char(src_rank));
        } else {
            // Different file (or only one other piece) — disambiguate by file
            disambiguation.push(file_char(src_file));
        }
    }

    // Build SAN
    let mut san = String::new();

    // Piece letter (none for pawn)
    if piece_type != PAWN_ID as usize {
        san.push(piece_letter(piece_type));
    }

    // Disambiguation
    san.push_str(&disambiguation);

    // Capture indicator
    let captured = game.board[dst as usize];
    if captured != 0 || is_en_passant(game, src, dst) {
        if piece_type == PAWN_ID as usize && disambiguation.is_empty() {
            // Pawn capture includes file
            san.push(file_char(src % 8));
        }
        san.push('x');
    }

    // Destination square
    san.push(file_char(dst % 8));
    san.push(rank_char(dst / 8));

    // Promotion
    if promo != 0 {
        san.push('=');
        san.push(piece_letter(promo.abs() as usize));
    }

    // Check / checkmate suffix (requires simulating the move)
    let mut game_copy = game.clone();
    crate::api::moves::do_move(&mut game_copy, src, dst, true);
    let opponent = -color;
    let in_check = is_in_check(&game_copy, opponent);
    let has_legal = has_any_legal_move(&mut game_copy, opponent);

    if in_check && !has_legal {
        san.push('#');
    } else if in_check {
        san.push('+');
    }

    san
}

fn file_char(file: i8) -> char {
    (b'a' + file as u8) as char
}

fn rank_char(rank: i8) -> char {
    (b'1' + rank as u8) as char
}

fn piece_letter(piece_type: usize) -> char {
    match piece_type {
        2 => 'N',
        3 => 'B',
        4 => 'R',
        5 => 'Q',
        6 => 'K',
        _ => '?',
    }
}

fn is_en_passant(game: &Game, src: i8, dst: i8) -> bool {
    let piece = game.board[src as usize];
    piece.abs() == PAWN_ID
        && game.en_passant_target == Some(dst)
}

fn has_any_legal_move(game: &mut Game, color: Color) -> bool {
    let moves = generate_pseudo_legal_moves(game, color);
    for mv in moves {
        let captured = game.board[mv.dst as usize];
        game.board[mv.dst as usize] = game.board[mv.src as usize];
        game.board[mv.src as usize] = 0;
        let legal = !is_in_check(game, color);
        game.board[mv.src as usize] = game.board[mv.dst as usize];
        game.board[mv.dst as usize] = captured;
        if legal {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// PGN assembler
// ---------------------------------------------------------------------------

/// PGN result strings
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PgnResult {
    WhiteWins,
    BlackWins,
    Draw,
    Unfinished,
}

impl PgnResult {
    pub fn as_str(&self) -> &'static str {
        match self {
            PgnResult::WhiteWins => "1-0",
            PgnResult::BlackWins => "0-1",
            PgnResult::Draw => "1/2-1/2",
            PgnResult::Unfinished => "*",
        }
    }
}

/// Assembles a PGN document from tags, moves, and a result.
#[derive(Clone, Debug)]
pub struct PgnAssembler {
    tags: BTreeMap<String, String>,
    moves: Vec<(u16, String, Option<String>)>, // (move_num, white_san, black_san)
    current_halfmove: u16,
    result: PgnResult,
}

impl PgnAssembler {
    pub fn new() -> Self {
        Self {
            tags: BTreeMap::new(),
            moves: Vec::new(),
            current_halfmove: 0,
            result: PgnResult::Unfinished,
        }
    }

    /// Add a tag pair (e.g., "White", "Alice")
    pub fn tag(&mut self, key: &str, value: &str) -> &mut Self {
        self.tags.insert(key.to_string(), value.to_string());
        self
    }

    /// Add a move SAN. Automatically alternates white/black.
    pub fn add_move(&mut self, san: String) -> &mut Self {
        if self.current_halfmove % 2 == 0 {
            // White's move — start a new full move
            let move_num = self.current_halfmove / 2 + 1;
            self.moves.push((move_num, san, None));
        } else {
            // Black's move — fill in the previous entry
            if let Some(last) = self.moves.last_mut() {
                last.2 = Some(san);
            }
        }
        self.current_halfmove += 1;
        self
    }

    /// Set the game result.
    pub fn set_result(&mut self, result: PgnResult) -> &mut Self {
        self.result = result;
        self
    }

    /// Format as a standard PGN string.
    pub fn to_string(&self) -> String {
        let mut out = String::new();

        // Tags
        for (k, v) in &self.tags {
            out.push_str(&format!("[{} \"{}\"]\n", k, v));
        }
        out.push('\n');

        // Moves (word-wrapped at 80 chars)
        let mut line_len = 0;
        for (num, white, black) in &self.moves {
            let white_str = format!("{}. {}", num, white);
            if line_len + white_str.len() + 1 > 80 {
                out.push('\n');
                line_len = 0;
            } else if line_len > 0 {
                out.push(' ');
                line_len += 1;
            }
            out.push_str(&white_str);
            line_len += white_str.len();

            if let Some(b) = black {
                let black_str = format!(" {}", b);
                if line_len + black_str.len() + 1 > 80 {
                    out.push('\n');
                    line_len = 0;
                } else {
                    out.push(' ');
                    line_len += 1;
                }
                out.push_str(b);
                line_len += b.len();
            }
        }

        // Result
        if line_len > 0 {
            out.push(' ');
        }
        out.push_str(self.result.as_str());
        out.push('\n');

        out
    }
}

impl Default for PgnAssembler {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// PGN parsing
// ---------------------------------------------------------------------------

/// Error type for PGN parsing failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PgnParseError {
    InvalidTag(String),
    InvalidSan(String),
    IllegalMove(String),
    EmptyGame,
    InvalidResult(String),
}

impl std::fmt::Display for PgnParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PgnParseError::InvalidTag(s) => write!(f, "Invalid PGN tag: {}", s),
            PgnParseError::InvalidSan(s) => write!(f, "Invalid SAN notation: {}", s),
            PgnParseError::IllegalMove(s) => write!(f, "Illegal move: {}", s),
            PgnParseError::EmptyGame => write!(f, "No moves found in PGN"),
            PgnParseError::InvalidResult(s) => write!(f, "Invalid game result: {}", s),
        }
    }
}

impl std::error::Error for PgnParseError {}

/// A parsed PGN game with tags and move list.
#[derive(Clone, Debug, Default)]
pub struct ParsedPgnGame {
    /// Tag pairs from the PGN header (e.g., "White", "Black", "Date").
    pub tags: BTreeMap<String, String>,
    /// Flat list of moves in SAN, one per ply (half-move).
    pub moves: Vec<String>,
    /// Game result string.
    pub result: String,
}

impl ParsedPgnGame {
    /// Get a tag value by key (case-insensitive).
    pub fn tag(&self, key: &str) -> Option<&str> {
        let key_lower = key.to_lowercase();
        self.tags
            .iter()
            .find(|(k, _)| k.to_lowercase() == key_lower)
            .map(|(_, v)| v.as_str())
    }

    /// Number of full moves (plies / 2, rounded up).
    pub fn full_move_count(&self) -> usize {
        (self.moves.len() + 1) / 2
    }
}

/// Parse a PGN string into a structured game.
///
/// # Arguments
/// * `text` - Raw PGN text (may include tags, moves, comments, variations)
///
/// # Returns
/// `ParsedPgnGame` on success, `PgnParseError` on failure.
pub fn parse_pgn(text: &str) -> Result<ParsedPgnGame, PgnParseError> {
    let mut game = ParsedPgnGame::default();

    // --- Phase 1: Parse tags ---

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Tag pair line: [Key "value"]
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let inner = &trimmed[1..trimmed.len() - 1];
            if let Some(space_pos) = inner.find(' ') {
                let key = inner[..space_pos].trim().to_string();
                let value = inner[space_pos..].trim().trim_matches('"').to_string();
                game.tags.insert(key, value);
            }
        }
    }

    // --- Phase 2: Strip tags, comments, variations ---
    let mut cleaned = String::new();
    let mut depth = 0; // { depth = comment level, ( depth = variation level
    let mut in_tag_block = false;

    for ch in text.chars() {
        match ch {
            '[' if depth == 0 && !in_tag_block => {
                in_tag_block = true;
            }
            ']' if in_tag_block => {
                in_tag_block = false;
            }
            '{' if !in_tag_block => {
                depth += 1;
            }
            '}' if !in_tag_block && depth > 0 => {
                depth -= 1;
            }
            '(' if !in_tag_block && depth == 0 => {
                depth += 1000; // variation depth
            }
            ')' if !in_tag_block && depth >= 1000 => {
                depth -= 1000;
            }
            _ if !in_tag_block && depth == 0 => {
                cleaned.push(ch);
            }
            _ => {}
        }
    }

    // --- Phase 3: Tokenize moves ---
    // Replace newlines with spaces, normalize whitespace
    let flat = cleaned.replace('\n', " ").replace('\r', " ");
    let tokens: Vec<&str> = flat.split_whitespace().collect();

    for token in tokens {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }

        // Skip move numbers like "1." or "1..."
        if token.ends_with('.') || token.ends_with("...") {
            continue;
        }

        // Game result
        if token == "1-0" || token == "0-1" || token == "1/2-1/2" || token == "*" {
            game.result = token.to_string();
            continue;
        }

        // SAN move
        if token.len() >= 2 {
            game.moves.push(token.to_string());
        }
    }

    if game.moves.is_empty() && game.result.is_empty() {
        return Err(PgnParseError::EmptyGame);
    }

    Ok(game)
}

/// Convert a SAN move string to engine coordinates (src, dst, promo).
///
/// # Arguments
/// * `game` - Current game state (before the move)
/// * `san` - SAN string like "Nf3", "exd5", "O-O", "e8=Q"
///
/// # Returns
/// `(src_idx, dst_idx, promo_piece_id)` or `PgnParseError`.
pub fn san_to_move(game: &mut Game, san: &str) -> Result<(i8, i8, i8), PgnParseError> {
    let color = if game.move_counter % 2 == 0 {
        COLOR_WHITE
    } else {
        COLOR_BLACK
    };

    // Handle castling
    let san_trimmed = san.trim_matches('+').trim_matches('#');
    if san_trimmed == "O-O" || san_trimmed == "0-0" {
        let src = if color == COLOR_WHITE { 4 } else { 60 };
        let dst = if color == COLOR_WHITE { 6 } else { 62 };
        return Ok((src, dst, 0));
    }
    if san_trimmed == "O-O-O" || san_trimmed == "0-0-0" {
        let src = if color == COLOR_WHITE { 4 } else { 60 };
        let dst = if color == COLOR_WHITE { 2 } else { 58 };
        return Ok((src, dst, 0));
    }

    // Extract promotion piece from "e8=Q" or "e8Q"
    let (san_body, promo) = if let Some(eq_pos) = san_trimmed.find('=') {
        let promo_char = san_trimmed.chars().nth(eq_pos + 1).unwrap_or('Q');
        let promo_id = char_to_piece_id(promo_char);
        (&san_trimmed[..eq_pos], promo_id)
    } else if san_trimmed.len() >= 3 {
        let last_char = san_trimmed.chars().last().unwrap();
        if "QRBN".contains(last_char) && san_trimmed.bytes().nth(san_trimmed.len() - 2).map(|b| b.is_ascii_digit()).unwrap_or(false) {
            let promo_id = char_to_piece_id(last_char);
            (&san_trimmed[..san_trimmed.len() - 1], promo_id)
        } else {
            (san_trimmed, 0)
        }
    } else {
        (san_trimmed, 0)
    };

    // Determine piece type, destination, and disambiguation
    let mut chars = san_body.chars().peekable();

    // Piece letter (or pawn if lowercase start)
    let piece_type = match chars.peek() {
        Some(&c) if c.is_ascii_uppercase() => {
            chars.next();
            char_to_piece_id(c)
        }
        _ => PAWN_ID,
    };

    // Parse the rest: may include file/rank disambiguation, capture 'x', destination
    let rest: String = chars.collect();
    let rest = rest.trim_matches('+').trim_matches('#');

    // Find the destination square (last file+rank pair)
    let mut dst_file: Option<i8> = None;
    let mut dst_rank: Option<i8> = None;
    let mut src_file: Option<i8> = None;
    let mut src_rank: Option<i8> = None;

    // Destination is always the last file+rank in `rest` (ignoring 'x').
    // Everything before the destination (and before 'x') is disambiguation.
    // This handles: "d4", "f3", "exd5", "bd2", "1d4", "a1d4", "xd5", etc.
    let has_capture = rest.contains('x');
    let rest_no_x: String = rest.chars().filter(|&c| c != 'x').collect();
    let rbytes = rest_no_x.as_bytes();

    if rbytes.len() >= 2 {
        let last = rbytes[rbytes.len() - 1] as char;
        let second_last = rbytes[rbytes.len() - 2] as char;
        if ('a'..='h').contains(&second_last) && ('1'..='8').contains(&last) {
            dst_file = Some((second_last as u8 - b'a') as i8);
            dst_rank = Some((last as u8 - b'1') as i8);
            let disambig = &rest_no_x[..rest_no_x.len() - 2];
            for c in disambig.chars() {
                if ('a'..='h').contains(&c) {
                    src_file = Some((c as u8 - b'a') as i8);
                } else if ('1'..='8').contains(&c) {
                    src_rank = Some((c as u8 - b'1') as i8);
                }
            }
        }
    }

    let _ = has_capture; // used for clarity, not needed in candidate filter

    let dst_file = dst_file.ok_or_else(|| PgnParseError::InvalidSan(san.to_string()))?;
    let dst_rank = dst_rank.ok_or_else(|| PgnParseError::InvalidSan(san.to_string()))?;
    let dst = dst_rank * 8 + dst_file;

    // Generate pseudo-legal moves and find the matching one
    let moves = generate_pseudo_legal_moves(game, color);
    let candidates: Vec<KK> = moves
        .into_iter()
        .filter(|m| {
            let piece = game.board[m.src as usize];
            if piece.abs() != piece_type {
                return false;
            }
            if m.dst != dst {
                return false;
            }
            // Check disambiguation
            if let Some(sf) = src_file {
                if m.src % 8 != sf {
                    return false;
                }
            }
            if let Some(sr) = src_rank {
                if m.src / 8 != sr {
                    return false;
                }
            }
            true
        })
        .collect();

    if candidates.is_empty() {
        return Err(PgnParseError::IllegalMove(san.to_string()));
    }

    if candidates.len() > 1 {
        // Try to use legality check to narrow down
        for cand in &candidates {
            if is_legal_move(game, cand.src, cand.dst, color) {
                return Ok((cand.src, cand.dst, promo));
            }
        }
        return Err(PgnParseError::IllegalMove(format!(
            "{} — ambiguous with {} candidates",
            san,
            candidates.len()
        )));
    }

    let m = candidates[0];
    Ok((m.src, m.dst, promo))
}

fn char_to_piece_id(c: char) -> i8 {
    match c {
        'N' | 'n' => KNIGHT_ID,
        'B' | 'b' => BISHOP_ID,
        'R' | 'r' => ROOK_ID,
        'Q' | 'q' => QUEEN_ID,
        'K' | 'k' => KING_ID,
        _ => PAWN_ID,
    }
}
