//! On-chain K=32 Elo rating calculation for player profile updates.
//!
//! Ratings are stored at ×100 centiscale (1200 Elo → 120000 stored).
//! K=32 → K_SCALED=3200; standard 400-point divisor → 40000 in centiscale.

const K_SCALED: f64 = 3200.0;   // K=32 × 100
const DIVISOR: f64 = 40000.0;   // 400 × 100
const ELO_FLOOR: f64 = 10000.0; // 100 Elo minimum (× 100)

/// Calculate K=32 Elo update for two players.
///
/// Both ratings are in centiscale (×100): 1200 Elo → 120000.0.
/// `sa` is white's score: 1.0 win, 0.5 draw, 0.0 loss.
///
/// Returns `(new_white_rating, new_black_rating)`.
pub fn calculate_elo_update(white_rating: f64, black_rating: f64, sa: f64) -> (f64, f64) {
    let ea = 1.0 / (1.0 + 10.0_f64.powf((black_rating - white_rating) / DIVISOR));
    let sb = 1.0 - sa;
    let eb = 1.0 - ea;
    let new_white = (white_rating + K_SCALED * (sa - ea)).max(ELO_FLOOR);
    let new_black = (black_rating + K_SCALED * (sb - eb)).max(ELO_FLOOR);
    (new_white, new_black)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_players_win_increases_rating() {
        let (w, b) = calculate_elo_update(120000.0, 120000.0, 1.0);
        assert!(w > 120000.0);
        assert!(b < 120000.0);
        assert!((w - 121600.0).abs() < 1.0, "expected +1600 centiscale (+16 Elo): got {}", w);
    }

    #[test]
    fn draw_between_equal_players_unchanged() {
        let (w, b) = calculate_elo_update(120000.0, 120000.0, 0.5);
        assert!((w - 120000.0).abs() < 1.0);
        assert!((b - 120000.0).abs() < 1.0);
    }

    #[test]
    fn floor_prevents_negative_ratings() {
        let (_, b) = calculate_elo_update(200000.0, 10000.0, 1.0);
        assert!(b >= ELO_FLOOR);
    }
}
