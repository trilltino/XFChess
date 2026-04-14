//! On-chain Glicko-2 rating calculation for player ELO updates.

/// Calculate Glicko-2 rating update for two players.
///
/// Returns the updated ratings for both players based on the game outcome.
/// Uses a simplified Glicko-2 calculation optimized for on-chain computation.
///
/// # Arguments
/// * `winner_rating` - Current rating of the winner
/// * `winner_rd` - Current rating deviation of the winner
/// * `loser_rating` - Current rating of the loser
/// * `loser_rd` - Current rating deviation of the loser
/// * `winner_outcome` - 1.0 for win, 0.5 for draw, 0.0 for loss
///
/// # Returns
/// A tuple of (new_winner_rating, new_winner_rd, new_loser_rating, new_loser_rd)
pub fn calculate_glicko2_update(
    winner_rating: f64,
    winner_rd: f64,
    loser_rating: f64,
    loser_rd: f64,
    winner_outcome: f64,
) -> (f64, f64, f64, f64) {
    // Simplified Glicko-2 calculation for on-chain efficiency
    // Based on Glicko-2 algorithm but optimized for compute units
    
    let loser_outcome = 1.0 - winner_outcome;
    
    // Calculate expected scores
    let expected_winner = calculate_expected_score(winner_rating, loser_rating, winner_rd, loser_rd);
    let expected_loser = 1.0 - expected_winner;
    
    // Calculate new rating deviations
    let winner_new_rd = calculate_new_rd(winner_rd);
    let loser_new_rd = calculate_new_rd(loser_rd);
    
    // Calculate new ratings using Glicko-2 formula
    let winner_new_rating = winner_rating + (winner_new_rd.powi(2) * (winner_outcome - expected_winner));
    let loser_new_rating = loser_rating + (loser_new_rd.powi(2) * (loser_outcome - expected_loser));
    
    (winner_new_rating, winner_new_rd, loser_new_rating, loser_new_rd)
}

/// Calculate expected score using Glicko-2 formula.
fn calculate_expected_score(rating_a: f64, rating_b: f64, rd_a: f64, _rd_b: f64) -> f64 {
    let q = 0.005756462; // ln(10) / 400
    let g = calculate_g(rd_a);
    let expected = 1.0 / (1.0 + (-g * q * (rating_a - rating_b)).exp());
    expected
}

/// Calculate the g function for Glicko-2.
fn calculate_g(rd: f64) -> f64 {
    1.0 / (1.0 + (3.0 * rd.powi(2) * (std::f64::consts::PI).powi(2)).sqrt())
}

/// Calculate new rating deviation after a game.
fn calculate_new_rd(rd: f64) -> f64 {
    let c: f64 = 50.0; // System constant for volatility
    let new_rd = ((rd.powi(2) + c.powi(2)).sqrt()).min(350.0);
    new_rd
}
