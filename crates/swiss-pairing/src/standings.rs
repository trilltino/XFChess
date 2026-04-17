use crate::{MatchResult, Pairing, SwissPlayer, SwissRound, StandingsEntry};

/// Calculate tournament standings
///
/// Tiebreak order:
/// 1. Buchholz (sum of opponents' scores)
/// 2. Sonneborn-Berger (sum of defeated opponents' scores + 0.5 * drawn opponents)
/// 3. Rating (higher rated player wins tie)
pub fn calculate_standings(
    players: &[SwissPlayer],
    rounds: &[SwissRound],
    results: &[(u8, u16, MatchResult)], // (round, board, result)
) -> Vec<StandingsEntry> {
    let mut entries: Vec<StandingsEntry> = players
        .iter()
        .map(|p| {
            let buchholz = calculate_buchholz(p, players, rounds, results);
            let sonneborn = calculate_sonneborn_berger(p, players, rounds, results);

            StandingsEntry {
                player_id: p.id.clone(),
                score: p.score,
                buchholz,
                sonneborn,
                rating: p.rating,
                rank: 0,
            }
        })
        .collect();

    // Sort by: score desc, buchholz desc, sonneborn desc, rating desc
    entries.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap()
            .then_with(|| b.buchholz.partial_cmp(&a.buchholz).unwrap())
            .then_with(|| b.sonneborn.partial_cmp(&a.sonneborn).unwrap())
            .then_with(|| b.rating.cmp(&a.rating))
    });

    // Assign ranks (handle ties)
    let mut current_rank = 1;
    let mut prev_score = None;
    let mut prev_buchholz = None;
    let mut prev_sonneborn = None;

    for entry in entries.iter_mut() {
        let is_tie = if let (Some(ps), Some(pb), Some(pso)) = (prev_score, prev_buchholz, prev_sonneborn) {
            entry.score == ps && entry.buchholz == pb && entry.sonneborn == pso
        } else {
            false
        };

        if !is_tie {
            entry.rank = current_rank;
        }

        prev_score = Some(entry.score);
        prev_buchholz = Some(entry.buchholz);
        prev_sonneborn = Some(entry.sonneborn);
        current_rank += 1;
    }

    entries
}

/// Calculate Buchholz tiebreak (sum of opponents' scores)
fn calculate_buchholz(
    player: &SwissPlayer,
    all_players: &[SwissPlayer],
    _rounds: &[SwissRound],
    _results: &[(u8, u16, MatchResult)],
) -> f64 {
    let mut sum = 0.0;

    for opponent_id in &player.opponents {
        if let Some(opponent) = all_players.iter().find(|p| &p.id == opponent_id) {
            sum += opponent.score;
        }
    }

    // Adjust for byes (count as draw = 0.5)
    sum += player.bye_count as f64 * 0.5;

    sum
}

/// Calculate Sonneborn-Berger tiebreak
/// Sum of scores of opponents defeated + 0.5 * scores of opponents drawn
fn calculate_sonneborn_berger(
    player: &SwissPlayer,
    all_players: &[SwissPlayer],
    rounds: &[SwissRound],
    results: &[(u8, u16, MatchResult)],
) -> f64 {
    let mut sum = 0.0;

    for (round, board, result) in results {
        // Find this player's result in this round
        let round_data = match rounds.iter().find(|r| r.round == *round) {
            Some(r) => r,
            None => continue,
        };

        let pairing = match round_data.pairings.iter().find(|p| p.board == *board) {
            Some(p) => p,
            None => continue,
        };

        // Check if this player is in this pairing
        let is_white = pairing.white == player.id;
        let is_black = pairing.black == player.id;

        if !is_white && !is_black {
            continue;
        }

        // Find opponent
        let opponent_id = if is_white {
            &pairing.black
        } else {
            &pairing.white
        };

        let opponent = match all_players.iter().find(|p| &p.id == opponent_id) {
            Some(p) => p,
            None => continue,
        };

        // Add to sum based on result
        match result {
            MatchResult::WhiteWin if is_white => sum += opponent.score,
            MatchResult::BlackWin if is_black => sum += opponent.score,
            MatchResult::Draw => sum += opponent.score * 0.5,
            _ => {} // Lost - no points
        }
    }

    sum
}

/// Update player scores based on results
pub fn update_scores(players: &mut [SwissPlayer], results: &[(u8, Pairing, MatchResult)]) {
    for (round, pairing, result) in results {
        if let Some(white) = players.iter_mut().find(|p| p.id == pairing.white) {
            white.score += result.white_score();
            white.opponents.push(pairing.black.clone());
            white.color_history.push(Color::White);
        }

        if let Some(black) = players.iter_mut().find(|p| p.id == pairing.black) {
            black.score += result.black_score();
            black.opponents.push(pairing.white.clone());
            black.color_history.push(Color::Black);
        }
    }
}

/// Get the final ranking for a player
pub fn get_player_rank(standings: &[StandingsEntry], player_id: &str) -> Option<u16> {
    standings
        .iter()
        .find(|s| s.player_id == player_id)
        .map(|s| s.rank)
}

/// Get top N players
pub fn get_top_players(standings: &[StandingsEntry], n: usize) -> Vec<&StandingsEntry> {
    standings.iter().take(n).collect()
}

use crate::Color;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Color, FloatStatus, SwissPlayer};

    fn test_player(id: &str, rating: u32, score: f64) -> SwissPlayer {
        SwissPlayer {
            id: id.to_string(),
            rating,
            score,
            color_history: Vec::new(),
            opponents: Vec::new(),
            bye_count: 0,
            float_status: FloatStatus::None,
        }
    }

    #[test]
    fn test_calculate_standings() {
        let players = vec![
            test_player("p1", 2000, 3.0),
            test_player("p2", 1900, 2.5),
            test_player("p3", 1800, 2.5),
            test_player("p4", 1700, 1.0),
        ];

        let rounds: Vec<SwissRound> = Vec::new();
        let results: Vec<(u8, u16, MatchResult)> = Vec::new();

        let standings = calculate_standings(&players, &rounds, &results);

        assert_eq!(standings[0].player_id, "p1");
        assert_eq!(standings[0].rank, 1);
        assert_eq!(standings[0].score, 3.0);

        // p2 and p3 are tied at 2.5
        assert_eq!(standings[1].score, 2.5);
        assert_eq!(standings[2].score, 2.5);
        // Higher rated gets better rank on tie
        assert!(standings[1].rating >= standings[2].rating);
    }

    #[test]
    fn test_update_scores() {
        let mut players = vec![
            test_player("p1", 2000, 0.0),
            test_player("p2", 1900, 0.0),
        ];

        let results = vec![(
            1,
            Pairing {
                white: "p1".to_string(),
                black: "p2".to_string(),
                board: 1,
            },
            MatchResult::WhiteWin,
        )];

        update_scores(&mut players, &results);

        assert_eq!(players[0].score, 1.0);
        assert_eq!(players[1].score, 0.0);
        assert_eq!(players[0].opponents, vec!["p2"]);
        assert_eq!(players[1].opponents, vec!["p1"]);
        assert_eq!(players[0].color_history, vec![Color::White]);
        assert_eq!(players[1].color_history, vec![Color::Black]);
    }

    #[test]
    fn test_get_top_players() {
        let standings = vec![
            StandingsEntry {
                player_id: "p1".to_string(),
                score: 5.0,
                buchholz: 10.0,
                sonneborn: 8.0,
                rating: 2000,
                rank: 1,
            },
            StandingsEntry {
                player_id: "p2".to_string(),
                score: 4.5,
                buchholz: 9.0,
                sonneborn: 7.0,
                rating: 1900,
                rank: 2,
            },
            StandingsEntry {
                player_id: "p3".to_string(),
                score: 4.0,
                buchholz: 8.0,
                sonneborn: 6.0,
                rating: 1800,
                rank: 3,
            },
        ];

        let top2 = get_top_players(&standings, 2);
        assert_eq!(top2.len(), 2);
        assert_eq!(top2[0].player_id, "p1");
        assert_eq!(top2[1].player_id, "p2");
    }
}
