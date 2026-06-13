//! Static Exchange Evaluation — the standard swap algorithm.
//!
//! Computes the material outcome of the capture sequence on a square assuming
//! both sides always recapture with their least valuable attacker. X-rays are
//! handled by recomputing sliding attacks against the shrinking occupancy.
//!
//! Replaces the previous victim−attacker approximation, which scored every
//! higher-takes-lower capture as losing even when the victim was undefended.

use crate::on_chain_attack::{
    bishop_attacks, rook_attacks, BLACK_PAWN_ATTACKS, KING_ATTACKS, KNIGHT_ATTACKS,
    WHITE_PAWN_ATTACKS,
};
use crate::constants::PAWN_ID;
use crate::types::*;

/// Piece values for exchange evaluation, indexed by piece type id (0 unused).
/// King is effectively infinite: capturing it ends the sequence decisively.
const SEE_VALUE: [i32; 7] = [0, 100, 320, 330, 500, 900, 20000];

/// All pieces (both colors) attacking `sq` under the given occupancy.
/// Piece bitboards from `game` are masked by `occ` at use, so pieces removed
/// during the swap stop attacking, and sliders behind them start to (x-ray).
fn attackers_to(game: &Game, sq: usize, occ: u64) -> u64 {
    let bishops_queens = game.white_bishops.0
        | game.black_bishops.0
        | game.white_queens.0
        | game.black_queens.0;
    let rooks_queens =
        game.white_rooks.0 | game.black_rooks.0 | game.white_queens.0 | game.black_queens.0;

    let mut attackers = 0u64;
    // A black pawn attacks sq from the squares a white pawn on sq would attack
    // (and vice versa) — the patterns are mirror images.
    attackers |= WHITE_PAWN_ATTACKS[sq] & game.black_pawns.0;
    attackers |= BLACK_PAWN_ATTACKS[sq] & game.white_pawns.0;
    attackers |= KNIGHT_ATTACKS[sq] & (game.white_knights.0 | game.black_knights.0);
    attackers |= KING_ATTACKS[sq] & (game.white_kings.0 | game.black_kings.0);
    attackers |= bishop_attacks(sq as u8, occ) & bishops_queens;
    attackers |= rook_attacks(sq as u8, occ) & rooks_queens;
    attackers & occ
}

/// Least valuable attacker of `white` among `attackers`. Returns (square, value).
fn least_valuable(game: &Game, attackers: u64, white: bool) -> Option<(usize, i32)> {
    let side = if white {
        [
            game.white_pawns.0,
            game.white_knights.0,
            game.white_bishops.0,
            game.white_rooks.0,
            game.white_queens.0,
            game.white_kings.0,
        ]
    } else {
        [
            game.black_pawns.0,
            game.black_knights.0,
            game.black_bishops.0,
            game.black_rooks.0,
            game.black_queens.0,
            game.black_kings.0,
        ]
    };
    for (i, bb) in side.iter().enumerate() {
        let cands = attackers & bb;
        if cands != 0 {
            return Some((cands.trailing_zeros() as usize, SEE_VALUE[i + 1]));
        }
    }
    None
}

/// Exchange value of `mv` in centipawns from the mover's perspective.
/// Positive: the capture sequence wins material; negative: it loses material.
pub fn see_value(game: &Game, mv: KK) -> i32 {
    let src = mv.src as usize;
    let dst = mv.dst as usize;
    let mover = game.board[src];
    if mover == 0 {
        return 0;
    }
    let mover_white = mover > 0;

    let mut occ = game.occupied.0;
    let first_victim = game.board[dst];
    let initial_gain = if first_victim != 0 {
        SEE_VALUE[first_victim.abs() as usize]
    } else if mover.abs() == PAWN_ID && game.en_passant_target == Some(mv.dst) {
        // En passant: the captured pawn is not on dst — remove it from occupancy.
        let cap_sq = if mover_white { dst - 8 } else { dst + 8 };
        occ &= !(1u64 << cap_sq);
        SEE_VALUE[PAWN_ID as usize]
    } else {
        0 // quiet move: the sequence can only lose the mover
    };

    // Swap list: gain[d] = best material balance after d recaptures.
    let mut gain = [0i32; 32];
    let mut d = 0usize;
    gain[0] = initial_gain;

    let mut on_dst_value = SEE_VALUE[mover.abs() as usize];
    occ &= !(1u64 << src); // mover leaves its square
    let mut side_white = !mover_white; // opponent recaptures first

    loop {
        let attackers = attackers_to(game, dst, occ);
        let side_occ = if side_white {
            game.occupied_white.0
        } else {
            game.occupied_black.0
        };
        let Some((lva_sq, lva_value)) = least_valuable(game, attackers & side_occ, side_white)
        else {
            break;
        };

        d += 1;
        if d >= gain.len() {
            break;
        }
        gain[d] = on_dst_value - gain[d - 1];

        // Both continuations already losing — no need to extend the sequence.
        if gain[d].max(-gain[d - 1]) < 0 {
            break;
        }

        on_dst_value = lva_value;
        occ &= !(1u64 << lva_sq);
        side_white = !side_white;
    }

    // Negamax the swap list backwards: each side may stop the sequence.
    while d > 0 {
        gain[d - 1] = -((-gain[d - 1]).max(gain[d]));
        d -= 1;
    }
    gain[0]
}

/// Convenience predicate: does the exchange meet `threshold`?
pub fn see(game: &Game, mv: KK, threshold: i32) -> bool {
    see_value(game, mv) >= threshold
}

#[cfg(test)]
#[cfg(feature = "std")]
mod tests {
    use super::*;
    use crate::api::game::game_from_fen;

    fn mv(src: i8, dst: i8) -> KK {
        KK::new(src, dst, 0, 0)
    }

    // Squares: a1=0 … h8=63; d2=11, d5=35, c6=42, e5=36, e2=12.

    #[test]
    fn rook_takes_undefended_pawn_wins() {
        // White Rd2 takes undefended pawn d5.
        let g = game_from_fen("1k6/8/8/3p4/8/8/3R4/3K4 w - - 0 1");
        assert!(see_value(&g, mv(11, 35)) > 0);
    }

    #[test]
    fn rook_takes_defended_pawn_loses() {
        // Pawn d5 defended by pawn c6: RxP, pxR → 100 - 500.
        let g = game_from_fen("1k6/8/2p5/3p4/8/8/3R4/3K4 w - - 0 1");
        assert_eq!(see_value(&g, mv(11, 35)), 100 - 500);
    }

    #[test]
    fn pawn_takes_defended_pawn_is_even() {
        // White pawn d4 takes pawn e5 defended by pawn d6: PxP, PxP → even.
        let g = game_from_fen("1k6/8/3p4/4p3/3P4/8/8/3K4 w - - 0 1");
        assert_eq!(see_value(&g, mv(27, 36)), 0);
    }

    #[test]
    fn xray_recapture_counts() {
        // White Rd1 (backed by Rd2... actually queen behind) — doubled rooks:
        // Rd2 takes d5 pawn, pawn c6 recaptures, Rd1 recaptures the pawn.
        // RxP(100) pxR(-500) RxP(100) → net 100 - 500 + 100 = -300 but the
        // negamax lets white stop after... sequence: white ends +100-500+100?
        // Standard result: white plays RxP only if forced sequence ≥ that;
        // with the backup rook the defender declines: SEE = 100 - 500 + 100
        // resolved by negamax = -300? No: defender captures only if it gains.
        // pxR wins 500 for 100 loss → defender takes; then RxP wins 100.
        // Net for white: 100 - 500 + 100 = -300. Defender could also decline,
        // leaving white +100. Negamax picks defender's best: -300 for white.
        // Therefore doubling doesn't rescue RxP against a pawn defender.
        let g = game_from_fen("1k6/8/2p5/3p4/8/8/3R4/3RK3 w - - 0 1");
        assert!(see_value(&g, mv(11, 35)) < 0);
    }

    #[test]
    fn quiet_move_to_attacked_square_loses_mover() {
        // White rook moves to d5 attacked by pawn c6 — quiet move, SEE = -500.
        let g = game_from_fen("1k6/8/2p5/8/8/8/3R4/3K4 w - - 0 1");
        assert_eq!(see_value(&g, mv(11, 35)), -500);
    }
}
