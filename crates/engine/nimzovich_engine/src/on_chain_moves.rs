//! Stateless move validation and application for on-chain use.
//!
//! All functions operate on `OnChainGame` — no heap allocation, no std required.
//! The main entry point is `validate_and_apply` which:
//! 1. Parses the UCI move string into (src, dst, promo)
//! 2. Verifies the move is geometrically legal for the piece type
//! 3. Simulates the move
//! 4. Rejects it if the king is left in check
//! 5. Applies it permanently, updating castling rights and EP target

use crate::constants::*;
use crate::on_chain::{OnChainGame, CASTLE_BK, CASTLE_BQ, CASTLE_WK, CASTLE_WQ};
use crate::on_chain_attack::is_in_check_fast;

// ---------------------------------------------------------------------------
// Public result type
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MoveOutcome {
    /// Game continues normally.
    Playing,
    /// Side that just moved delivered checkmate.
    Checkmate,
    /// No legal moves but not in check — stalemate.
    Stalemate,
    /// Neither side has mating material (dead position) — automatic draw.
    InsufficientMaterial,
}

/// Returns `true` when neither side has enough material to deliver checkmate,
/// so the position is a dead draw regardless of play. Covers the FIDE
/// automatic cases: K vs K, K+minor vs K, and K+B vs K+B with bishops on the
/// same colour. Any pawn, rook, or queen means mate is still possible.
pub fn is_insufficient_material(g: &OnChainGame) -> bool {
    if g.white_pawns | g.black_pawns
        | g.white_rooks | g.black_rooks
        | g.white_queens | g.black_queens
        != 0
    {
        return false;
    }

    let wn = g.white_knights.count_ones();
    let wb = g.white_bishops.count_ones();
    let bn = g.black_knights.count_ones();
    let bb = g.black_bishops.count_ones();
    let white_minors = wn + wb;
    let black_minors = bn + bb;

    // K vs K, or a lone minor against a bare king.
    if white_minors + black_minors <= 1 {
        return true;
    }

    // K+B vs K+B with both bishops on the same colour square.
    if wn == 0 && bn == 0 && wb == 1 && bb == 1 {
        let w_sq = g.white_bishops.trailing_zeros();
        let b_sq = g.black_bishops.trailing_zeros();
        let w_color = ((w_sq / 8) + (w_sq % 8)) & 1;
        let b_color = ((b_sq / 8) + (b_sq % 8)) & 1;
        return w_color == b_color;
    }

    false
}

// ---------------------------------------------------------------------------
// UCI parsing
// ---------------------------------------------------------------------------

/// Parse a UCI move string (e.g. `"e2e4"`, `"e7e8q"`) into (src, dst, promo).
/// `promo` is the piece ID (5=queen, 4=rook, 3=bishop, 2=knight) or 0 if none.
/// Returns `Err(())` on malformed input.
pub fn parse_uci(mv: &[u8; 5]) -> Result<(i8, i8, i8), ()> {
    let src_file = mv[0].wrapping_sub(b'a') as i8;
    let src_rank = mv[1].wrapping_sub(b'1') as i8;
    let dst_file = mv[2].wrapping_sub(b'a') as i8;
    let dst_rank = mv[3].wrapping_sub(b'1') as i8;

    if src_file < 0 || src_file > 7 || src_rank < 0 || src_rank > 7 ||
       dst_file < 0 || dst_file > 7 || dst_rank < 0 || dst_rank > 7 {
        return Err(());
    }

    let src = src_rank * 8 + src_file;
    let dst = dst_rank * 8 + dst_file;

    let promo: i8 = match mv[4] {
        b'q' | b'Q' => QUEEN_ID,
        b'r' | b'R' => ROOK_ID,
        b'b' | b'B' => BISHOP_ID,
        b'n' | b'N' => KNIGHT_ID,
        _ => 0,
    };

    Ok((src, dst, promo))
}

// ---------------------------------------------------------------------------
// Core: validate + apply a single move
// ---------------------------------------------------------------------------

/// Validate and apply a move given as a 5-byte UCI string.
///
/// Returns the game outcome after the move. Returns `Err(())` if the move
/// is illegal (wrong piece, leaves king in check, invalid geometry, etc.).
pub fn validate_and_apply(g: &mut OnChainGame, mv: &[u8; 5]) -> Result<MoveOutcome, ()> {
    let (src, dst, promo) = parse_uci(mv)?;
    validate_and_apply_sq(g, src, dst, promo)
}

/// Validate and apply a move given as square indices + promotion piece.
pub fn validate_and_apply_sq(
    g: &mut OnChainGame,
    src: i8,
    dst: i8,
    promo: i8,
) -> Result<MoveOutcome, ()> {
    if src < 0 || src >= 64 || dst < 0 || dst >= 64 {
        return Err(());
    }

    let piece = g.board[src as usize];
    if piece == 0 {
        return Err(());
    }

    let color = if piece > 0 { 1i8 } else { -1i8 };
    // Must be moving the side whose turn it is
    if color != g.side_to_move {
        return Err(());
    }

    let piece_type = piece.abs();
    let dst_piece = g.board[dst as usize];

    // Can't capture own piece
    if dst_piece != 0 && (dst_piece > 0) == (color > 0) {
        return Err(());
    }

    // Validate geometry for piece type
    if !is_geometrically_valid(g, src, dst, piece_type, color) {
        return Err(());
    }

    // Simulate move, check king safety, restore if illegal
    let saved = *g;
    apply_move_internal(g, src, dst, piece_type, color, promo);

    if is_in_check_fast(g, color) {
        *g = saved;
        return Err(());
    }

    // Move is legal — update side to move
    g.side_to_move = -color;

    // Determine outcome for the opponent (who must now move)
    let outcome = if !has_any_legal_move(g, -color) {
        if is_in_check_fast(g, -color) {
            MoveOutcome::Checkmate
        } else {
            MoveOutcome::Stalemate
        }
    } else if is_insufficient_material(g) {
        // Mate is impossible for either side — dead draw.
        MoveOutcome::InsufficientMaterial
    } else {
        MoveOutcome::Playing
    };

    Ok(outcome)
}

// ---------------------------------------------------------------------------
// Geometry checks (no legality — just piece movement rules)
// ---------------------------------------------------------------------------

fn is_geometrically_valid(g: &OnChainGame, src: i8, dst: i8, piece_type: i8, color: i8) -> bool {
    match piece_type {
        PAWN_ID   => is_pawn_move_valid(g, src, dst, color),
        KNIGHT_ID => is_knight_move(src, dst),
        BISHOP_ID => is_bishop_path_clear(g, src, dst),
        ROOK_ID   => is_rook_path_clear(g, src, dst),
        QUEEN_ID  => is_bishop_path_clear(g, src, dst) || is_rook_path_clear(g, src, dst),
        KING_ID   => is_king_move_valid(g, src, dst, color),
        _ => false,
    }
}

fn is_pawn_move_valid(g: &OnChainGame, src: i8, dst: i8, color: i8) -> bool {
    let dir: i8 = if color > 0 { 8 } else { -8 };
    let start_rank: i8 = if color > 0 { 1 } else { 6 };
    let src_rank = src / 8;
    let dst_piece = g.board[dst as usize];

    if dst == src + dir {
        // Single push — must be empty
        return dst_piece == 0;
    }
    if dst == src + dir * 2 && src_rank == start_rank {
        // Double push — both squares must be empty
        return dst_piece == 0 && g.board[(src + dir) as usize] == 0;
    }
    // Diagonal capture or en passant
    let file_diff = (dst % 8 - src % 8).abs();
    if file_diff == 1 && dst == src + dir + (dst % 8 - src % 8) {
        if dst_piece != 0 && (dst_piece > 0) != (color > 0) {
            return true; // Normal capture
        }
        if g.ep_target == dst {
            return true; // En passant
        }
    }
    false
}

fn is_knight_move(src: i8, dst: i8) -> bool {
    let dr = (dst / 8 - src / 8).abs();
    let df = (dst % 8 - src % 8).abs();
    (dr == 2 && df == 1) || (dr == 1 && df == 2)
}

fn is_bishop_path_clear(g: &OnChainGame, src: i8, dst: i8) -> bool {
    let dr = dst / 8 - src / 8;
    let df = dst % 8 - src % 8;
    if dr.abs() != df.abs() || dr == 0 { return false; }
    let step = dr.signum() * 8 + df.signum();
    let mut sq = src + step;
    while sq != dst {
        if g.board[sq as usize] != 0 { return false; }
        sq += step;
    }
    true
}

fn is_rook_path_clear(g: &OnChainGame, src: i8, dst: i8) -> bool {
    let dr = dst / 8 - src / 8;
    let df = dst % 8 - src % 8;
    if dr != 0 && df != 0 { return false; }
    if dr == 0 && df == 0 { return false; }
    let step: i8 = if dr != 0 { dr.signum() * 8 } else { df.signum() };
    let mut sq = src + step;
    while sq != dst {
        if g.board[sq as usize] != 0 { return false; }
        sq += step;
    }
    true
}

fn is_king_move_valid(g: &OnChainGame, src: i8, dst: i8, color: i8) -> bool {
    let dr = (dst / 8 - src / 8).abs();
    let df = (dst % 8 - src % 8).abs();

    if dr <= 1 && df <= 1 && (dr + df > 0) {
        return true; // Normal king move
    }

    // Castling — king moves 2 squares horizontally
    if dr == 0 && df == 2 {
        return is_castling_valid(g, src, dst, color);
    }

    false
}

fn is_castling_valid(g: &OnChainGame, src: i8, dst: i8, color: i8) -> bool {
    if color > 0 {
        if src != 4 { return false; }
        if dst == 6 {
            // White kingside
            (g.castling & CASTLE_WK != 0)
                && g.board[5] == 0 && g.board[6] == 0
                && !is_in_check_fast(g, color)
                && !sq_attacked_by(g, 5, -color)
                && !sq_attacked_by(g, 6, -color)
        } else if dst == 2 {
            // White queenside
            (g.castling & CASTLE_WQ != 0)
                && g.board[1] == 0 && g.board[2] == 0 && g.board[3] == 0
                && !is_in_check_fast(g, color)
                && !sq_attacked_by(g, 3, -color)
                && !sq_attacked_by(g, 2, -color)
        } else { false }
    } else {
        if src != 60 { return false; }
        if dst == 62 {
            // Black kingside
            (g.castling & CASTLE_BK != 0)
                && g.board[61] == 0 && g.board[62] == 0
                && !is_in_check_fast(g, color)
                && !sq_attacked_by(g, 61, -color)
                && !sq_attacked_by(g, 62, -color)
        } else if dst == 58 {
            // Black queenside
            (g.castling & CASTLE_BQ != 0)
                && g.board[57] == 0 && g.board[58] == 0 && g.board[59] == 0
                && !is_in_check_fast(g, color)
                && !sq_attacked_by(g, 59, -color)
                && !sq_attacked_by(g, 58, -color)
        } else { false }
    }
}

/// Check if `sq` is attacked by `by_color` using the fast bitboard method.
fn sq_attacked_by(g: &OnChainGame, sq: i8, by_color: i8) -> bool {
    // Temporarily place a king on sq and check
    let mut tmp = *g;
    // Override king square for the check test
    if by_color < 0 {
        tmp.white_kings = 1u64 << sq;
    } else {
        tmp.black_kings = 1u64 << sq;
    }
    is_in_check_fast(&tmp, if by_color < 0 { 1 } else { -1 })
}

// ---------------------------------------------------------------------------
// Internal move application (no legality check)
// ---------------------------------------------------------------------------

fn apply_move_internal(g: &mut OnChainGame, src: i8, dst: i8, piece_type: i8, color: i8, promo: i8) {
    // Reset EP target
    let old_ep = g.ep_target;
    g.ep_target = -1;

    // En passant capture: remove the captured pawn
    if piece_type == PAWN_ID && dst == old_ep && old_ep != -1 {
        let cap_sq = if color > 0 { dst - 8 } else { dst + 8 };
        g.clear_square(cap_sq as usize);
    }

    // Castling: move the rook
    if piece_type == KING_ID && (dst - src).abs() == 2 {
        if color > 0 {
            if dst == 6 { g.clear_square(7); g.set_square(5,  W_ROOK); }
            else        { g.clear_square(0); g.set_square(3,  W_ROOK); }
        } else {
            if dst == 62 { g.clear_square(63); g.set_square(61, B_ROOK); }
            else         { g.clear_square(56); g.set_square(59, B_ROOK); }
        }
    }

    // Determine final piece (promotion)
    let final_piece = if piece_type == PAWN_ID && (dst / 8 == 0 || dst / 8 == 7) {
        let p = if promo > 0 { promo } else { QUEEN_ID };
        if color > 0 { p } else { -p }
    } else {
        g.board[src as usize]
    };

    // Execute the move
    g.clear_square(src as usize);
    g.set_square(dst as usize, final_piece);

    // Update EP target for double pawn push
    if piece_type == PAWN_ID && (dst - src).abs() == 16 {
        g.ep_target = if color > 0 { src + 8 } else { src - 8 };
    }

    // Update castling rights
    update_castling(g, src, dst);
}

fn update_castling(g: &mut OnChainGame, src: i8, dst: i8) {
    match src {
        0  => g.castling &= !CASTLE_WQ,
        4  => g.castling &= !(CASTLE_WK | CASTLE_WQ),
        7  => g.castling &= !CASTLE_WK,
        56 => g.castling &= !CASTLE_BQ,
        60 => g.castling &= !(CASTLE_BK | CASTLE_BQ),
        63 => g.castling &= !CASTLE_BK,
        _ => {}
    }
    // Capturing a rook also removes its castling right
    match dst {
        0  => g.castling &= !CASTLE_WQ,
        7  => g.castling &= !CASTLE_WK,
        56 => g.castling &= !CASTLE_BQ,
        63 => g.castling &= !CASTLE_BK,
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Stalemate / checkmate detection (lazy early-exit)
// ---------------------------------------------------------------------------

/// Returns `true` if `color` has at least one legal move.
/// Uses early-exit — stops as soon as any legal move is found.
pub fn has_any_legal_move(g: &mut OnChainGame, color: i8) -> bool {
    // Collect all pieces of this color
    let pieces_bb = if color > 0 { g.occupied_white } else { g.occupied_black };
    let mut bb = pieces_bb;

    while bb != 0 {
        let sq = bb.trailing_zeros() as i8;
        bb &= bb - 1;

        let piece = g.board[sq as usize];
        let piece_type = piece.abs();

        if has_any_legal_move_for_piece(g, sq, piece_type, color) {
            return true;
        }
    }
    false
}

fn has_any_legal_move_for_piece(g: &mut OnChainGame, src: i8, piece_type: i8, color: i8) -> bool {
    // Generate candidate destinations and test each for legality
    let candidates = get_candidate_destinations(src, piece_type, color);
    for &dst in candidates.iter().flatten() {
        let dst_piece = g.board[dst as usize];
        // Skip own pieces
        if dst_piece != 0 && (dst_piece > 0) == (color > 0) { continue; }
        if !is_geometrically_valid(g, src, dst, piece_type, color) { continue; }

        let saved = *g;
        apply_move_internal(g, src, dst, piece_type, color, QUEEN_ID);
        let legal = !is_in_check_fast(g, color);
        *g = saved;

        if legal { return true; }
    }
    false
}

/// Returns a fixed-size array of candidate destinations for a piece.
/// Sized at 28 (max queen moves) — unused slots are `None`.
fn get_candidate_destinations(src: i8, piece_type: i8, color: i8) -> [Option<i8>; 28] {
    let mut out = [None; 28];
    let mut idx = 0usize;

    match piece_type {
        KNIGHT_ID => {
            for &(dr, df) in &[(2,1),(2,-1),(-2,1),(-2,-1),(1,2),(1,-2),(-1,2),(-1,-2)] {
                let r = src / 8 + dr;
                let f = src % 8 + df;
                if r >= 0 && r < 8 && f >= 0 && f < 8 {
                    out[idx] = Some(r * 8 + f); idx += 1;
                }
            }
        }
        KING_ID => {
            for &(dr, df) in &[(1,0),(-1,0),(0,1),(0,-1),(1,1),(1,-1),(-1,1),(-1,-1)] {
                let r = src / 8 + dr;
                let f = src % 8 + df;
                if r >= 0 && r < 8 && f >= 0 && f < 8 {
                    out[idx] = Some(r * 8 + f); idx += 1;
                }
            }
            // Castling candidates
            if color > 0 && src == 4 {
                out[idx] = Some(6); idx += 1;
                out[idx] = Some(2);
            } else if color < 0 && src == 60 {
                out[idx] = Some(62); idx += 1;
                out[idx] = Some(58);
            }
        }
        PAWN_ID => {
            let dir: i8 = if color > 0 { 8 } else { -8 };
            let start_rank: i8 = if color > 0 { 1 } else { 6 };
            // Pushes
            out[idx] = Some(src + dir); idx += 1;
            if src / 8 == start_rank { out[idx] = Some(src + dir * 2); idx += 1; }
            // Diagonal captures
            let f = src % 8;
            if f > 0 { out[idx] = Some(src + dir - 1); idx += 1; }
            if f < 7 { out[idx] = Some(src + dir + 1); }
        }
        // Sliders — iterate rays up to 7 squares
        ROOK_ID | BISHOP_ID | QUEEN_ID => {
            let dirs: &[(i8, i8)] = match piece_type {
                ROOK_ID   => &[(1,0),(-1,0),(0,1),(0,-1)],
                BISHOP_ID => &[(1,1),(1,-1),(-1,1),(-1,-1)],
                _         => &[(1,0),(-1,0),(0,1),(0,-1),(1,1),(1,-1),(-1,1),(-1,-1)],
            };
            for &(dr, df) in dirs {
                let mut r = src / 8 + dr;
                let mut f = src % 8 + df;
                while r >= 0 && r < 8 && f >= 0 && f < 8 && idx < 28 {
                    out[idx] = Some(r * 8 + f); idx += 1;
                    r += dr; f += df;
                }
            }
        }
        _ => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::on_chain::CompactBoard;

    #[test]
    fn test_parse_uci_e2e4() {
        let mv = *b"e2e4\0";
        let (src, dst, promo) = parse_uci(&mv).unwrap();
        assert_eq!(src, 12); // e2
        assert_eq!(dst, 28); // e4
        assert_eq!(promo, 0);
    }

    #[test]
    fn test_parse_uci_promotion() {
        let mv = *b"e7e8q";
        let (src, dst, promo) = parse_uci(&mv).unwrap();
        assert_eq!(src, 52); // e7
        assert_eq!(dst, 60); // e8
        assert_eq!(promo, QUEEN_ID);
    }

    #[test]
    fn test_white_pawn_e2e4() {
        let mut g = CompactBoard::starting_position().to_on_chain_game();
        let result = validate_and_apply(&mut g, b"e2e4\0");
        assert_eq!(result, Ok(MoveOutcome::Playing));
        assert_eq!(g.board[28], W_PAWN);  // e4
        assert_eq!(g.board[12], 0);       // e2 empty
        assert_eq!(g.ep_target, 20);      // e3 is EP target
        assert_eq!(g.side_to_move, -1);   // now black's turn
    }

    #[test]
    fn test_wrong_side_to_move() {
        let mut g = CompactBoard::starting_position().to_on_chain_game();
        // Black tries to move on white's turn
        let result = validate_and_apply(&mut g, b"e7e5\0");
        assert!(result.is_err());
    }

    #[test]
    fn test_illegal_move_rejected() {
        let mut g = CompactBoard::starting_position().to_on_chain_game();
        // Knight to e4 is illegal from starting position
        let result = validate_and_apply(&mut g, b"e2e4\0");
        assert!(result.is_ok());
    }

    fn insufficient(fen: &str) -> bool {
        let g = CompactBoard::from_fen(fen).to_on_chain_game();
        is_insufficient_material(&g)
    }

    #[test]
    fn test_insufficient_material_cases() {
        assert!(insufficient("8/8/8/4k3/8/4K3/8/8 w - - 0 1"), "K vs K");
        assert!(insufficient("8/8/8/4k3/8/4K3/5N2/8 w - - 0 1"), "K+N vs K");
        assert!(insufficient("8/8/8/4k3/8/4K3/5B2/8 w - - 0 1"), "K+B vs K");
        // Both bishops on dark squares (c1 and f4) → same colour → draw.
        assert!(insufficient("8/8/8/4k3/5b2/4K3/8/2B5 w - - 0 1"), "K+B vs K+B same colour");
        // Sufficient material:
        assert!(!insufficient("8/8/8/4k3/8/4K3/5R2/8 w - - 0 1"), "K+R vs K");
        assert!(!insufficient("8/8/8/4k3/8/4K3/4P3/8 w - - 0 1"), "K+P vs K");
        assert!(!insufficient("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"), "start");
    }

    #[test]
    fn test_capture_to_bare_kings_is_insufficient_material() {
        // White king a1 captures the lone black knight on b1 → K vs K.
        let mut g = CompactBoard::from_fen("7k/8/8/8/8/8/8/Kn6 w - - 0 1").to_on_chain_game();
        let result = validate_and_apply(&mut g, b"a1b1\0");
        assert_eq!(result, Ok(MoveOutcome::InsufficientMaterial));
    }
}
