//! Integer Elo on the centiscale ratings stored in `PlayerProfile` (rating x 100).
//!
//! - Expected score comes from a table with linear interpolation (`table.rs`), so the update
//!   uses only integer arithmetic and is bit-exact on every target.
//! - K depends on experience and strength, as in FIDE rules: 40 while a rating is provisional
//!   (fewer than 30 rated games in the time control), 20 afterwards, 10 from 2400.
//! - A per-game weight lets the caller reduce a result's effect (repeated opponents).
//!
//! Scores are in ten-thousandths: 10000 = win, 5000 = draw, 0 = loss.
use super::table::{EXPECTED_STRONG, MAX_GAP_POINTS, STEP_POINTS};

pub const SCORE_WIN: u32 = 10_000;
pub const SCORE_DRAW: u32 = 5_000;
pub const SCORE_LOSS: u32 = 0;

pub const PROVISIONAL_GAMES: u8 = 30;
pub const K_PROVISIONAL: u32 = 40;
pub const K_STANDARD: u32 = 20;
pub const K_MASTER: u32 = 10;
/// 2400 points in centiscale.
pub const MASTER_THRESHOLD: u32 = 240_000;
/// 100 points in centiscale; ratings never fall below this.
pub const ELO_FLOOR: u32 = 10_000;

const CENTI: u32 = 100;
const STEP: u32 = STEP_POINTS * CENTI;
const MAX_GAP: u32 = MAX_GAP_POINTS * CENTI;

/// One player's side of a rating update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Side {
    pub rating: u32,
    /// Rated games already played in this time control.
    pub games: u8,
    /// Percentage (0-100) of the normal change that applies to this game.
    pub weight_pct: u32,
}

pub fn k_factor(rating: u32, games: u8) -> u32 {
    if games < PROVISIONAL_GAMES {
        K_PROVISIONAL
    } else if rating >= MASTER_THRESHOLD {
        K_MASTER
    } else {
        K_STANDARD
    }
}

/// Expected score of `own` against `opponent`, in ten-thousandths.
pub fn expected_score(own: u32, opponent: u32) -> u32 {
    let (stronger, gap) = if own >= opponent {
        (true, own - opponent)
    } else {
        (false, opponent - own)
    };
    let gap = gap.min(MAX_GAP);
    let index = (gap / STEP) as usize;
    let fraction = gap % STEP;
    let low = u32::from(EXPECTED_STRONG[index]);
    let high = u32::from(EXPECTED_STRONG[(index + 1).min(EXPECTED_STRONG.len() - 1)]);
    // The table rises with the gap, so `high >= low`.
    let strong = low + (high - low) * fraction / STEP;
    if stronger {
        strong
    } else {
        SCORE_WIN - strong
    }
}

/// `side`'s rating after scoring `score` against an opponent rated `opponent_rating`.
pub fn rating_after(side: Side, opponent_rating: u32, score: u32) -> u32 {
    let expected = i64::from(expected_score(side.rating, opponent_rating));
    let k = i64::from(k_factor(side.rating, side.games));
    // K points x 100 centi x (score - expected) / 10000, then the weight.
    let delta = k * (i64::from(score) - expected) * i64::from(side.weight_pct.min(100)) / 10_000;
    (i64::from(side.rating) + delta).clamp(i64::from(ELO_FLOOR), i64::from(u32::MAX)) as u32
}

/// New ratings for white and black after white scored `white_score`.
pub fn calculate_elo_update(white: Side, black: Side, white_score: u32) -> (u32, u32) {
    (
        rating_after(white, black.rating, white_score),
        rating_after(black, white.rating, SCORE_WIN - white_score),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn side(rating: u32, games: u8) -> Side {
        Side {
            rating,
            games,
            weight_pct: 100,
        }
    }

    #[test]
    fn equal_established_players_move_ten_points_on_a_win() {
        let (w, b) = calculate_elo_update(side(120_000, 50), side(120_000, 50), SCORE_WIN);
        assert_eq!(w, 121_000, "K=20, expected 0.5: +10 points");
        assert_eq!(b, 119_000);
    }

    #[test]
    fn provisional_players_move_twice_as_fast() {
        let (w, _) = calculate_elo_update(side(120_000, 0), side(120_000, 50), SCORE_WIN);
        assert_eq!(w, 122_000, "K=40: +20 points");
        let (w, _) = calculate_elo_update(side(120_000, PROVISIONAL_GAMES), side(120_000, 50), SCORE_WIN);
        assert_eq!(w, 121_000, "provisional ends at 30 games");
    }

    #[test]
    fn masters_move_slowly() {
        assert_eq!(k_factor(MASTER_THRESHOLD, 60), K_MASTER);
        assert_eq!(k_factor(MASTER_THRESHOLD - 1, 60), K_STANDARD);
        assert_eq!(k_factor(MASTER_THRESHOLD, 3), K_PROVISIONAL);
    }

    #[test]
    fn draw_between_equals_changes_nothing() {
        let (w, b) = calculate_elo_update(side(150_000, 40), side(150_000, 40), SCORE_DRAW);
        assert_eq!((w, b), (150_000, 150_000));
    }

    #[test]
    fn expected_scores_are_complementary_and_monotonic() {
        let mut previous = 0;
        for gap in (0..=90_000).step_by(37) {
            let strong = expected_score(100_000 + gap, 100_000);
            let weak = expected_score(100_000, 100_000 + gap);
            assert_eq!(strong + weak, SCORE_WIN, "gap {gap}");
            assert!(strong >= previous, "gap {gap}");
            previous = strong;
        }
        assert_eq!(expected_score(150_000, 150_000), SCORE_DRAW);
    }

    #[test]
    fn expected_score_stays_within_one_part_in_a_thousand_of_the_float_formula() {
        for gap_points in 0..=800u32 {
            let own = 150_000 + gap_points * 100;
            let table = f64::from(expected_score(own, 150_000)) / 10_000.0;
            let exact = 1.0 / (1.0 + 10f64.powf(-f64::from(gap_points) / 400.0));
            assert!((table - exact).abs() < 0.001, "gap {gap_points}: {table} vs {exact}");
        }
    }

    #[test]
    fn gaps_beyond_the_table_use_its_last_value() {
        assert_eq!(expected_score(400_000, 100_000), expected_score(180_000, 100_000));
        assert_eq!(expected_score(100_000, 400_000), SCORE_WIN - 9_901);
    }

    #[test]
    fn an_upset_pays_more_than_an_expected_win() {
        let (upset, _) = calculate_elo_update(side(120_000, 50), side(160_000, 50), SCORE_WIN);
        let (expected, _) = calculate_elo_update(side(160_000, 50), side(120_000, 50), SCORE_WIN);
        assert!(upset - 120_000 > expected - 160_000);
    }

    #[test]
    fn floor_prevents_ratings_below_one_hundred() {
        let (_, b) = calculate_elo_update(side(200_000, 50), side(ELO_FLOOR, 0), SCORE_WIN);
        assert_eq!(b, ELO_FLOOR);
    }

    #[test]
    fn weight_scales_the_change_and_zero_freezes_it() {
        let full = rating_after(side(120_000, 50), 120_000, SCORE_WIN);
        let half = rating_after(Side { weight_pct: 50, ..side(120_000, 50) }, 120_000, SCORE_WIN);
        let none = rating_after(Side { weight_pct: 0, ..side(120_000, 50) }, 120_000, SCORE_WIN);
        assert_eq!((full - 120_000, half - 120_000, none), (1_000, 500, 120_000));
    }

    #[test]
    fn extreme_ratings_never_overflow() {
        let top = side(u32::MAX, 0);
        let (w, b) = calculate_elo_update(top, side(ELO_FLOOR, 0), SCORE_WIN);
        assert!(w >= u32::MAX - 100 && b == ELO_FLOOR);
    }
}
