use crate::{MatchResult, Pairing, StandingsEntry, SwissPlayer, SwissRound};

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
            .total_cmp(&a.score)
            .then_with(|| b.buchholz.total_cmp(&a.buchholz))
            .then_with(|| b.sonneborn.total_cmp(&a.sonneborn))
            .then_with(|| b.rating.cmp(&a.rating))
    });

    // Standard competition ranking: players level on score and both tie-breaks
    // share a rank, and the next rank skips past them (1, 2, 2, 4).
    for i in 0..entries.len() {
        let tied = i > 0 && {
            let (prev, entry) = (&entries[i - 1], &entries[i]);
            (prev.score, prev.buchholz, prev.sonneborn)
                == (entry.score, entry.buchholz, entry.sonneborn)
        };
        entries[i].rank = if tied {
            entries[i - 1].rank
        } else {
            u16::try_from(i + 1).unwrap_or(u16::MAX)
        };
    }

    entries
}

fn calculate_buchholz(
    player: &SwissPlayer,
    all_players: &[SwissPlayer],
    _rounds: &[SwissRound],
    _results: &[(u8, u16, MatchResult)],
) -> f64 {
    let mut sum = 0.0;

    for opponent_id in &player.opponents {
        // Withdrawn opponents are excluded from Buchholz (gap 2)
        if let Some(opponent) = all_players
            .iter()
            .find(|p| &p.id == opponent_id && !p.withdrawn)
        {
            sum += opponent.score;
        }
    }

    // Adjust for byes (count as draw = 0.5)
    sum += player.bye_count() as f64 * 0.5;

    sum
}

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

        let opponent_id = if is_white {
            &pairing.black
        } else {
            &pairing.white
        };

        let opponent = match all_players.iter().find(|p| &p.id == opponent_id) {
            Some(p) => p,
            None => continue,
        };

        match result {
            MatchResult::WhiteWin if is_white => sum += opponent.score,
            MatchResult::BlackWin if is_black => sum += opponent.score,
            MatchResult::Draw => sum += opponent.score * 0.5,
            _ => {} // Lost - no points
        }
    }

    sum
}

pub fn update_scores(players: &mut [SwissPlayer], results: &[(u8, Pairing, MatchResult)]) {
    for (_, pairing, result) in results {
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

pub fn get_player_rank(standings: &[StandingsEntry], player_id: &str) -> Option<u16> {
    standings
        .iter()
        .find(|s| s.player_id == player_id)
        .map(|s| s.rank)
}

pub fn get_top_players(standings: &[StandingsEntry], n: usize) -> Vec<&StandingsEntry> {
    standings.iter().take(n).collect()
}

use crate::Color;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Color, SwissPlayer};

    fn test_player(id: &str, rating: u32, score: f64) -> SwissPlayer {
        SwissPlayer {
            id: id.to_string(),
            rating,
            score,
            color_history: Vec::new(),
            opponents: Vec::new(),
            bye_rounds: Vec::new(),
            float_history: Vec::new(),
            absent: false,
            withdrawn: false,
            forfeit_round: None,
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
    fn tied_players_share_a_rank_and_the_next_rank_skips() {
        // p2 and p3 are level on score and both tie-breaks (no rounds played).
        let players = vec![
            test_player("p1", 2000, 3.0),
            test_player("p2", 1900, 2.5),
            test_player("p3", 1800, 2.5),
            test_player("p4", 1700, 1.0),
        ];

        let standings = calculate_standings(&players, &[], &[]);

        let ranks: Vec<u16> = standings.iter().map(|s| s.rank).collect();
        assert_eq!(ranks, vec![1, 2, 2, 4]);
    }

    #[test]
    fn test_update_scores() {
        let mut players = vec![test_player("p1", 2000, 0.0), test_player("p2", 1900, 0.0)];

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
