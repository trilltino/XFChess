//! Full position evaluation: PeSTO tapered material + piece-square tables.
//!
//! Replaces the previous hand-rolled eval (material + custom PSTs + mobility).
//! The mobility term generated all pseudo-legal moves for BOTH sides at every
//! node — the single largest NPS cost in the engine — and the untuned tables
//! lost decisively to tuned opponents at equal depth (0-7 vs Rustic Alpha 1 at
//! fixed depth 5). PeSTO's Texel-tuned tables are the standard known-good
//! drop-in; see `pesto.rs` for sources and orientation notes.

use super::pesto::{EG_PESTO, EG_VALUE, MG_PESTO, MG_VALUE, PHASE_INC, PHASE_MAX};
use crate::types::*;

/// Evaluate the position. Positive = good for White, in centipawns.
pub fn evaluate_position(game: &Game) -> i16 {
    let mut mg = 0i32;
    let mut eg = 0i32;
    let mut phase = 0i32;

    for square in 0..64usize {
        let piece = game.board[square];
        if piece == 0 {
            continue;
        }
        let pt = (piece.abs() - 1) as usize; // 0..=5 = P,N,B,R,Q,K

        if piece > 0 {
            // White: tables are printed rank-8-first, our a1 = 0 → flip.
            let idx = square ^ 56;
            mg += MG_VALUE[pt] + MG_PESTO[pt][idx];
            eg += EG_VALUE[pt] + EG_PESTO[pt][idx];
        } else {
            // Black: direct index reads the table as black's mirrored view.
            mg -= MG_VALUE[pt] + MG_PESTO[pt][square];
            eg -= EG_VALUE[pt] + EG_PESTO[pt][square];
        }
        phase += PHASE_INC[pt];
    }

    // Tapered blend, phase capped so early promotions don't overflow it.
    let mg_phase = phase.min(PHASE_MAX);
    let eg_phase = PHASE_MAX - mg_phase;
    let mut score = (mg * mg_phase + eg * eg_phase) / PHASE_MAX;

    // Mop-up: in late endgames with a decisive material edge, reward driving
    // the losing king to the edge/corner and marching our king toward it.
    // Without this, KQK/KRK-style wins shuffle within the PST optimum instead
    // of making mating progress.
    if mg_phase <= 6 && score.abs() >= 400 {
        let wk = game.white_kings.0.trailing_zeros() as i32;
        let bk = game.black_kings.0.trailing_zeros() as i32;
        if wk < 64 && bk < 64 {
            let loser_k = if score > 0 { bk } else { wk };
            // Manhattan distance of the losing king from the board centre.
            let (lf, lr) = (loser_k % 8, loser_k / 8);
            let centre_dist = (2 * lf - 7).abs() / 2 + (2 * lr - 7).abs() / 2;
            // Proximity of the two kings (winner wants to close in).
            let king_dist = ((wk % 8) - (bk % 8)).abs().max(((wk / 8) - (bk / 8)).abs());
            let mop = 10 * centre_dist + 4 * (7 - king_dist);
            score += if score > 0 { mop } else { -mop };
        }
    }

    score.clamp(i16::MIN as i32 + 1, i16::MAX as i32 - 1) as i16
}

#[cfg(test)]
#[cfg(feature = "std")]
mod tests {
    use super::*;
    use crate::api::game::{game_from_fen, new_game};

    #[test]
    fn startpos_is_symmetric() {
        let game = new_game();
        assert_eq!(evaluate_position(&game), 0);
    }

    #[test]
    fn extra_queen_is_decisive() {
        // White queen vs nothing extra for black
        let game = game_from_fen("rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert!(
            evaluate_position(&game) > 800,
            "missing black queen should be ~+9"
        );
    }

    #[test]
    fn advanced_pawn_beats_home_pawn() {
        // White pawn on e7 (about to promote) vs on e2
        let advanced = game_from_fen("4k3/4P3/8/8/8/8/8/4K3 w - - 0 1");
        let home = game_from_fen("4k3/8/8/8/8/8/4P3/4K3 w - - 0 1");
        assert!(
            evaluate_position(&advanced) > evaluate_position(&home) + 50,
            "e7 pawn must out-eval e2 pawn (orientation check)"
        );
    }

    #[test]
    fn eval_is_color_symmetric() {
        // Mirrored positions must produce negated scores.
        let w = game_from_fen("4k3/8/8/8/8/2N5/8/4K3 w - - 0 1"); // white knight c3
        let b = game_from_fen("4k3/8/2n5/8/8/8/8/4K3 w - - 0 1"); // black knight c6
        assert_eq!(evaluate_position(&w), -evaluate_position(&b));
    }
}
