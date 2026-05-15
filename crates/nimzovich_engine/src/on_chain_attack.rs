//! Bitboard attack tables and O(1) check detection for on-chain validation.
//!
//! Uses the "reverse attack" (or "super-piece") technique:
//! place a hypothetical queen/knight/pawn on the king square and check
//! whether it attacks any enemy piece of that type. This avoids scanning
//! all 64 squares and is a single integer AND per piece type.
//!
//! All tables are computed at compile time with `const fn` — zero runtime cost.

use crate::on_chain::OnChainGame;

// ---------------------------------------------------------------------------
// Precomputed attack tables
// ---------------------------------------------------------------------------

/// Knight attack bitboard for each of the 64 squares.
const KNIGHT_ATTACKS: [u64; 64] = compute_knight_attacks();

/// King attack bitboard for each of the 64 squares.
const KING_ATTACKS: [u64; 64] = compute_king_attacks();

/// White pawn attack bitboard from each square (captures diagonally forward).
const WHITE_PAWN_ATTACKS: [u64; 64] = compute_pawn_attacks(true);

/// Black pawn attack bitboard from each square (captures diagonally forward).
const BLACK_PAWN_ATTACKS: [u64; 64] = compute_pawn_attacks(false);

// ---------------------------------------------------------------------------
// Compile-time table generation
// ---------------------------------------------------------------------------

const fn compute_knight_attacks() -> [u64; 64] {
    let mut table = [0u64; 64];
    let mut sq = 0usize;
    while sq < 64 {
        let r = (sq / 8) as i32;
        let f = (sq % 8) as i32;
        let deltas: [(i32, i32); 8] = [
            (2, 1), (2, -1), (-2, 1), (-2, -1),
            (1, 2), (1, -2), (-1, 2), (-1, -2),
        ];
        let mut bb = 0u64;
        let mut i = 0;
        while i < 8 {
            let (dr, df) = deltas[i];
            let nr = r + dr;
            let nf = f + df;
            if nr >= 0 && nr < 8 && nf >= 0 && nf < 8 {
                bb |= 1u64 << (nr * 8 + nf);
            }
            i += 1;
        }
        table[sq] = bb;
        sq += 1;
    }
    table
}

const fn compute_king_attacks() -> [u64; 64] {
    let mut table = [0u64; 64];
    let mut sq = 0usize;
    while sq < 64 {
        let r = (sq / 8) as i32;
        let f = (sq % 8) as i32;
        let deltas: [(i32, i32); 8] = [
            (1,0),(-1,0),(0,1),(0,-1),(1,1),(1,-1),(-1,1),(-1,-1)
        ];
        let mut bb = 0u64;
        let mut i = 0;
        while i < 8 {
            let (dr, df) = deltas[i];
            let nr = r + dr;
            let nf = f + df;
            if nr >= 0 && nr < 8 && nf >= 0 && nf < 8 {
                bb |= 1u64 << (nr * 8 + nf);
            }
            i += 1;
        }
        table[sq] = bb;
        sq += 1;
    }
    table
}

const fn compute_pawn_attacks(white: bool) -> [u64; 64] {
    let mut table = [0u64; 64];
    let mut sq = 0usize;
    while sq < 64 {
        let r = (sq / 8) as i32;
        let f = (sq % 8) as i32;
        let mut bb = 0u64;
        let dr: i32 = if white { 1 } else { -1 };
        let nr = r + dr;
        if nr >= 0 && nr < 8 {
            if f - 1 >= 0 { bb |= 1u64 << (nr * 8 + f - 1); }
            if f + 1 < 8  { bb |= 1u64 << (nr * 8 + f + 1); }
        }
        table[sq] = bb;
        sq += 1;
    }
    table
}

// ---------------------------------------------------------------------------
// Sliding piece attack generation (classical fill)
// ---------------------------------------------------------------------------

/// Generate rook attacks from `sq` given occupancy `occ`.
/// Uses classical ray-casting — no magic bitboards needed at this scale.
#[inline]
pub fn rook_attacks(sq: u8, occ: u64) -> u64 {
    ray_north(sq, occ)
    | ray_south(sq, occ)
    | ray_east(sq, occ)
    | ray_west(sq, occ)
}

/// Generate bishop attacks from `sq` given occupancy `occ`.
#[inline]
pub fn bishop_attacks(sq: u8, occ: u64) -> u64 {
    ray_ne(sq, occ) | ray_nw(sq, occ) | ray_se(sq, occ) | ray_sw(sq, occ)
}

/// Generate queen attacks (rook + bishop).
#[inline]
pub fn queen_attacks(sq: u8, occ: u64) -> u64 {
    rook_attacks(sq, occ) | bishop_attacks(sq, occ)
}

// Individual ray directions using classical o^(o-2r) fill.
// We use simple loop-based ray generation to avoid magic numbers.

#[inline]
fn ray_north(sq: u8, occ: u64) -> u64 {
    let mut attacks = 0u64;
    let mut s = sq as i32 + 8;
    while s < 64 {
        attacks |= 1u64 << s;
        if (occ >> s) & 1 != 0 { break; }
        s += 8;
    }
    attacks
}

#[inline]
fn ray_south(sq: u8, occ: u64) -> u64 {
    let mut attacks = 0u64;
    let mut s = sq as i32 - 8;
    while s >= 0 {
        attacks |= 1u64 << s;
        if (occ >> s) & 1 != 0 { break; }
        s -= 8;
    }
    attacks
}

#[inline]
fn ray_east(sq: u8, occ: u64) -> u64 {
    let mut attacks = 0u64;
    let mut s = sq as i32 + 1;
    while s < 64 && s % 8 != 0 {
        attacks |= 1u64 << s;
        if (occ >> s) & 1 != 0 { break; }
        s += 1;
    }
    attacks
}

#[inline]
fn ray_west(sq: u8, occ: u64) -> u64 {
    let mut attacks = 0u64;
    let mut s = sq as i32 - 1;
    while s >= 0 && (s + 1) % 8 != 0 {
        attacks |= 1u64 << s;
        if (occ >> s) & 1 != 0 { break; }
        s -= 1;
    }
    attacks
}

#[inline]
fn ray_ne(sq: u8, occ: u64) -> u64 {
    let mut attacks = 0u64;
    let mut s = sq as i32 + 9;
    while s < 64 && s % 8 != 0 {
        attacks |= 1u64 << s;
        if (occ >> s) & 1 != 0 { break; }
        s += 9;
    }
    attacks
}

#[inline]
fn ray_nw(sq: u8, occ: u64) -> u64 {
    let mut attacks = 0u64;
    let mut s = sq as i32 + 7;
    while s < 64 && (s + 1) % 8 != 0 {
        attacks |= 1u64 << s;
        if (occ >> s) & 1 != 0 { break; }
        s += 7;
    }
    attacks
}

#[inline]
fn ray_se(sq: u8, occ: u64) -> u64 {
    let mut attacks = 0u64;
    let mut s = sq as i32 - 7;
    while s >= 0 && s % 8 != 0 {
        attacks |= 1u64 << s;
        if (occ >> s) & 1 != 0 { break; }
        s -= 7;
    }
    attacks
}

#[inline]
fn ray_sw(sq: u8, occ: u64) -> u64 {
    let mut attacks = 0u64;
    let mut s = sq as i32 - 9;
    while s >= 0 && (s + 1) % 8 != 0 {
        attacks |= 1u64 << s;
        if (occ >> s) & 1 != 0 { break; }
        s -= 9;
    }
    attacks
}

// Unused generic ray helpers kept for completeness
#[allow(dead_code)]
fn ray_attacks(sq: u8, occ: u64, shift: u32, _mask: u64) -> u64 {
    ray_north(sq, occ) // simplified — proper dispatch done above
}

// ---------------------------------------------------------------------------
// O(1) check detection
// ---------------------------------------------------------------------------

/// Returns `true` if the king of `color` is in check.
///
/// Uses the reverse-attack (super-piece) technique — no board scan.
/// Each piece-type check is a single bitboard AND.
#[inline]
pub fn is_in_check_fast(g: &OnChainGame, color: i8) -> bool {
    let king_sq = match g.king_square(color) {
        Some(s) => s,
        None => return false,
    };

    let occ = g.occupied;

    if color > 0 {
        // White king — check for black attackers
        let opp_rq = g.black_rooks | g.black_queens;
        let opp_bq = g.black_bishops | g.black_queens;
        rook_attacks(king_sq, occ) & opp_rq != 0
            || bishop_attacks(king_sq, occ) & opp_bq != 0
            || KNIGHT_ATTACKS[king_sq as usize] & g.black_knights != 0
            || WHITE_PAWN_ATTACKS[king_sq as usize] & g.black_pawns != 0
            || KING_ATTACKS[king_sq as usize] & g.black_kings != 0
    } else {
        // Black king — check for white attackers
        let opp_rq = g.white_rooks | g.white_queens;
        let opp_bq = g.white_bishops | g.white_queens;
        rook_attacks(king_sq, occ) & opp_rq != 0
            || bishop_attacks(king_sq, occ) & opp_bq != 0
            || KNIGHT_ATTACKS[king_sq as usize] & g.white_knights != 0
            || BLACK_PAWN_ATTACKS[king_sq as usize] & g.white_pawns != 0
            || KING_ATTACKS[king_sq as usize] & g.white_kings != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::on_chain::CompactBoard;

    #[test]
    fn test_no_check_starting_position() {
        let g = CompactBoard::starting_position().to_on_chain_game();
        assert!(!is_in_check_fast(&g, 1),  "White should not be in check at start");
        assert!(!is_in_check_fast(&g, -1), "Black should not be in check at start");
    }

    #[test]
    fn test_knight_attacks_e4() {
        // e4 = square 28; knights attack from c3(18),c5(34),d2(11),d6(43),f2(13),f6(45),g3(22),g5(38)
        let attacks = KNIGHT_ATTACKS[28];
        assert!(attacks & (1 << 18) != 0, "c3");
        assert!(attacks & (1 << 34) != 0, "c5");
        assert!(attacks & (1 << 11) != 0, "d2");
        assert!(attacks & (1 << 43) != 0, "d6");
    }

    #[test]
    fn test_rook_attacks_open_file() {
        // Rook on e4 (sq 28), empty board
        let attacks = rook_attacks(28, 1u64 << 28);
        // Should see the full e-file and 4th rank minus e4 itself
        assert!(attacks & (1 << 20) != 0, "e3");
        assert!(attacks & (1 << 36) != 0, "e5");
        assert!(attacks & (1 << 24) != 0, "a4");
        assert!(attacks & (1 << 31) != 0, "h4");
    }
}
