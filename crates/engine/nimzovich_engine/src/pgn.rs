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
/// * `game` - Game state *before* the move is applied. Takes `&mut Game` so the
///   check/checkmate suffix can be derived via a trial `do_move` + restore in
///   place — cloning `Game` here would also deep-copy its multi-GB `tt` (see
///   [`crate::api::moves::is_legal_move_unchecked`] for the same pattern), which
///   made this function the dominant per-move cost when called from the hot
///   path (`ChessEngine::move_to_san`, invoked on every human/AI move).
/// * `src` - Source square (0-63)
/// * `dst` - Destination square (0-63)
/// * `promo` - Promotion piece ID (0 = none, 2-5 = N/B/R/Q)
///
/// # Returns
/// SAN string (e.g., "Nxf7+", "e8=Q", "O-O-O")
pub fn move_to_san(game: &mut Game, src: i8, dst: i8, promo: i8) -> String {
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
            m.dst == dst && game.board[m.src as usize].abs() == piece_type as i8 && m.src != src
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

    // Check / checkmate suffix — simulate the move in place and restore
    // afterward rather than cloning `Game` (whose `tt` field alone is ~2 GB).
    let board_before = game.board;
    let ep_before = game.en_passant_target;
    let halfmove_before = game.halfmove_clock;
    let move_counter_before = game.move_counter;
    let wk_before = game.white_king_has_moved;
    let bk_before = game.black_king_has_moved;
    let wr0_before = game.white_rook_0_has_moved;
    let wr7_before = game.white_rook_7_has_moved;
    let br56_before = game.black_rook_56_has_moved;
    let br63_before = game.black_rook_63_has_moved;
    #[cfg(feature = "search")]
    let hash_before = game.current_hash;

    crate::api::moves::do_move(game, src, dst, true);
    let opponent = -color;
    let in_check = is_in_check(game, opponent);
    let has_legal = has_any_legal_move(game, opponent);

    game.board = board_before;
    game.en_passant_target = ep_before;
    game.halfmove_clock = halfmove_before;
    game.move_counter = move_counter_before;
    game.white_king_has_moved = wk_before;
    game.black_king_has_moved = bk_before;
    game.white_rook_0_has_moved = wr0_before;
    game.white_rook_7_has_moved = wr7_before;
    game.black_rook_56_has_moved = br56_before;
    game.black_rook_63_has_moved = br63_before;
    crate::board::init_bitboards(game);
    #[cfg(feature = "search")]
    {
        game.hash_history.pop();
        game.current_hash = hash_before;
    }

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
    piece.abs() == PAWN_ID && game.en_passant_target == Some(dst)
}

fn has_any_legal_move(game: &mut Game, color: Color) -> bool {
    let moves = generate_pseudo_legal_moves(game, color);
    for mv in moves {
        let captured = game.board[mv.dst as usize];
        game.board[mv.dst as usize] = game.board[mv.src as usize];
        game.board[mv.src as usize] = 0;
        // is_in_check reads bitboards, not the mailbox — they must be kept in
        // sync with the trial board or every iteration silently checks the
        // pre-loop position instead of the candidate move.
        crate::board::init_bitboards(game);
        let legal = !is_in_check(game, color);
        game.board[mv.src as usize] = game.board[mv.dst as usize];
        game.board[mv.dst as usize] = captured;
        crate::board::init_bitboards(game);
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
    /// Per-ply annotations extracted by `parse_pgn_annotated`.
    /// Index 0 = starting position, index N = after ply N.
    /// Empty when loaded via `parse_pgn`.
    pub per_ply_annotations: Vec<PerPlyAnnotation>,
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
        if "QRBN".contains(last_char)
            && san_trimmed
                .bytes()
                .nth(san_trimmed.len() - 2)
                .map(|b| b.is_ascii_digit())
                .unwrap_or(false)
        {
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

// ---------------------------------------------------------------------------
// Annotation types
// ---------------------------------------------------------------------------

/// Move quality classification parsed from PGN suffixes or NAG codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MoveQuality {
    #[default]
    Normal,
    Good,        // !
    Brilliant,   // !!
    Interesting, // !?
    Dubious,     // ?!
    Mistake,     // ?
    Blunder,     // ??
}

/// Per-ply annotation data extracted from PGN move comments.
/// Index i in `ParsedPgnGame::per_ply_annotations` = state after ply i.
#[derive(Clone, Debug, Default)]
pub struct PerPlyAnnotation {
    /// Arrows: (from_file, from_rank, to_file, to_rank, color_kind)
    /// color_kind: 0=green, 1=orange/yellow, 2=blue
    pub arrows: Vec<(u8, u8, u8, u8, u8)>,
    /// Square highlights: (file, rank, color_kind)
    pub highlights: Vec<(u8, u8, u8)>,
    /// Move quality badge for this ply's move (Normal = no badge)
    pub quality: MoveQuality,
    /// Free-text comment after this move in the PGN
    pub comment: Option<String>,
}

// ---------------------------------------------------------------------------
// Annotated PGN parser
// ---------------------------------------------------------------------------

/// Parse a PGN string, preserving per-move comments and extracting
/// `[%cal]` / `[%csl]` arrow/highlight annotations and quality suffixes.
///
/// Returns the same `ParsedPgnGame` struct but with `per_ply_annotations`
/// populated. Moves in `moves` have quality suffixes stripped so they can be
/// passed directly to `san_to_move`.
pub fn parse_pgn_annotated(text: &str) -> Result<ParsedPgnGame, PgnParseError> {
    let mut game = ParsedPgnGame::default();

    // Phase 1: Parse tags (identical to parse_pgn)
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let inner = &trimmed[1..trimmed.len() - 1];
            if let Some(space_pos) = inner.find(' ') {
                let key = inner[..space_pos].trim().to_string();
                let value = inner[space_pos..].trim().trim_matches('"').to_string();
                game.tags.insert(key, value);
            }
        }
    }

    // Phase 2: Tokenize — SAN tokens, comment blocks, NAG codes
    enum RawToken {
        San(String),
        Comment(String),
        Nag(u8),
        Result,
    }

    let mut raw: Vec<RawToken> = Vec::new();
    let mut chars = text.chars().peekable();
    let mut var_depth: i32 = 0;

    while let Some(&ch) = chars.peek() {
        match ch {
            '[' if var_depth == 0 => {
                chars.next();
                for c in chars.by_ref() {
                    if c == ']' {
                        break;
                    }
                }
            }
            '{' if var_depth == 0 => {
                chars.next();
                let mut comment = String::new();
                for c in chars.by_ref() {
                    if c == '}' {
                        break;
                    }
                    comment.push(c);
                }
                raw.push(RawToken::Comment(comment));
            }
            '(' if var_depth == 0 => {
                var_depth += 1;
                chars.next();
            }
            '(' => {
                var_depth += 1;
                chars.next();
            }
            ')' if var_depth > 0 => {
                var_depth -= 1;
                chars.next();
            }
            _ if var_depth > 0 => {
                chars.next();
            }
            '$' => {
                chars.next();
                let mut num = String::new();
                while chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    num.push(chars.next().unwrap());
                }
                if let Ok(n) = num.parse::<u8>() {
                    raw.push(RawToken::Nag(n));
                }
            }
            c if c.is_whitespace() => {
                chars.next();
            }
            _ => {
                let mut word = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() || c == '{' || c == '(' || c == ')' || c == '$' {
                        break;
                    }
                    word.push(c);
                    chars.next();
                }
                if word.is_empty() {
                    continue;
                }
                if word.ends_with('.') || word.ends_with("...") {
                    continue;
                }
                if matches!(word.as_str(), "1-0" | "0-1" | "1/2-1/2" | "*") {
                    game.result = word.clone();
                    raw.push(RawToken::Result);
                    continue;
                }
                if word.len() >= 2 {
                    raw.push(RawToken::San(word));
                }
            }
        }
    }

    // Phase 3: Build (clean_san, annotation) pairs
    // Ply 0 = starting position
    game.per_ply_annotations.push(PerPlyAnnotation::default());

    let mut i = 0;
    while i < raw.len() {
        match &raw[i] {
            RawToken::San(san_raw) => {
                let (clean, quality) = strip_quality_suffix(san_raw);
                let mut ann = PerPlyAnnotation {
                    quality,
                    ..Default::default()
                };

                // Consume trailing comments / NAGs before the next SAN / Result
                let mut j = i + 1;
                while j < raw.len() {
                    match &raw[j] {
                        RawToken::Comment(c) => {
                            ann.comment = Some(c.clone());
                            parse_comment_annotations(c, &mut ann);
                            j += 1;
                        }
                        RawToken::Nag(n) => {
                            let q = nag_to_quality(*n);
                            if q != MoveQuality::Normal {
                                ann.quality = q;
                            }
                            j += 1;
                        }
                        _ => break,
                    }
                }

                game.moves.push(clean);
                game.per_ply_annotations.push(ann);
                i = j;
            }
            _ => {
                i += 1;
            }
        }
    }

    if game.moves.is_empty() && game.result.is_empty() {
        return Err(PgnParseError::EmptyGame);
    }
    Ok(game)
}

// ---------------------------------------------------------------------------
// Annotation helpers (private)
// ---------------------------------------------------------------------------

fn strip_quality_suffix(san: &str) -> (String, MoveQuality) {
    // Strip trailing check/mate first, then quality
    let s = san.trim_end_matches('+').trim_end_matches('#');
    if s.ends_with("!!") {
        return (s[..s.len() - 2].to_string(), MoveQuality::Brilliant);
    }
    if s.ends_with("??") {
        return (s[..s.len() - 2].to_string(), MoveQuality::Blunder);
    }
    if s.ends_with("!?") {
        return (s[..s.len() - 2].to_string(), MoveQuality::Interesting);
    }
    if s.ends_with("?!") {
        return (s[..s.len() - 2].to_string(), MoveQuality::Dubious);
    }
    if s.ends_with('!') {
        return (s[..s.len() - 1].to_string(), MoveQuality::Good);
    }
    if s.ends_with('?') {
        return (s[..s.len() - 1].to_string(), MoveQuality::Mistake);
    }
    (s.to_string(), MoveQuality::Normal)
}

fn nag_to_quality(nag: u8) -> MoveQuality {
    match nag {
        1 => MoveQuality::Good,
        2 => MoveQuality::Mistake,
        3 => MoveQuality::Brilliant,
        4 => MoveQuality::Blunder,
        5 => MoveQuality::Interesting,
        6 => MoveQuality::Dubious,
        _ => MoveQuality::Normal,
    }
}

fn parse_comment_annotations(comment: &str, ann: &mut PerPlyAnnotation) {
    // Extract [%cmd args] directives
    let mut rest = comment;
    while let Some(start) = rest.find("[%") {
        rest = &rest[start + 1..]; // skip '['
        let end = match rest.find(']') {
            Some(e) => e,
            None => break,
        };
        let directive = &rest[..end];
        rest = &rest[end + 1..];

        let (name, args) = if let Some(sp) = directive.find(' ') {
            (&directive[..sp], directive[sp + 1..].trim())
        } else {
            (directive, "")
        };

        match name {
            "%cal" | "%arrow" => {
                for pair in args.split(',') {
                    let pair = pair.trim();
                    if pair.len() < 5 {
                        continue;
                    }
                    let kind = pgn_color_to_kind(pair.chars().next().unwrap_or('G'));
                    let data = &pair[1..];
                    if data.len() < 4 {
                        continue;
                    }
                    let mut dc = data.chars();
                    let ff = dc.next().and_then(file_char_to_u8);
                    let fr = dc.next().and_then(rank_char_to_u8);
                    let tf = dc.next().and_then(file_char_to_u8);
                    let tr = dc.next().and_then(rank_char_to_u8);
                    if let (Some(ff), Some(fr), Some(tf), Some(tr)) = (ff, fr, tf, tr) {
                        ann.arrows.push((ff, fr, tf, tr, kind));
                    }
                }
            }
            "%csl" => {
                for sq in args.split(',') {
                    let sq = sq.trim();
                    if sq.len() < 3 {
                        continue;
                    }
                    let kind = pgn_color_to_kind(sq.chars().next().unwrap_or('G'));
                    let mut sc = sq[1..].chars();
                    let f = sc.next().and_then(file_char_to_u8);
                    let r = sc.next().and_then(rank_char_to_u8);
                    if let (Some(f), Some(r)) = (f, r) {
                        ann.highlights.push((f, r, kind));
                    }
                }
            }
            _ => {}
        }
    }
}

fn pgn_color_to_kind(c: char) -> u8 {
    match c {
        'Y' | 'y' | 'R' | 'r' => 1, // orange/yellow/red → kind 1
        'B' | 'b' => 2,             // blue
        _ => 0,                     // green (G) and anything else
    }
}

fn file_char_to_u8(c: char) -> Option<u8> {
    if ('a'..='h').contains(&c) {
        Some(c as u8 - b'a')
    } else {
        None
    }
}

fn rank_char_to_u8(c: char) -> Option<u8> {
    if ('1'..='8').contains(&c) {
        Some(c as u8 - b'1')
    } else {
        None
    }
}

#[cfg(test)]
mod san_tests {
    use super::*;
    use crate::api::game::new_game;

    fn sq(file: i8, rank: i8) -> i8 {
        rank * 8 + file
    }

    /// Fool's Mate: 1. f3 e5 2. g4 Qh4# — the final move must be reported
    /// with '#', not '+'. Regression test for `move_to_san` no longer
    /// cloning `Game` and for the `has_any_legal_move` bitboard-staleness fix
    /// (both used to be exercised only via a full `game.clone()`).
    #[test]
    fn fools_mate_reports_checkmate_suffix() {
        let mut game = new_game();
        crate::api::moves::do_move(&mut game, sq(5, 1), sq(5, 2), true); // f2-f3
        crate::api::moves::do_move(&mut game, sq(4, 6), sq(4, 4), true); // e7-e5
        crate::api::moves::do_move(&mut game, sq(6, 1), sq(6, 3), true); // g2-g4

        let san = move_to_san(&mut game, sq(3, 7), sq(7, 3), 0); // Qd8-h4
        assert_eq!(san, "Qh4#");
    }

    /// A check with a legal king escape must be reported with '+', not '#'.
    #[test]
    fn check_with_escape_reports_check_suffix_not_mate() {
        let mut game = crate::api::game::game_from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1");
        let san = move_to_san(&mut game, sq(4, 0), sq(4, 6), 0); // Re1-e7+
        assert_eq!(san, "Re7+");
    }
}
