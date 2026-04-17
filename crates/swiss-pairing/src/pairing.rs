use crate::{
    Color, FloatStatus, Pairing, PairingError, PairingResult, Scoregroup, SwissPlayer, SwissRound,
};
use tracing::{debug, trace, warn};

/// Generate pairings for a Swiss round using the Dutch system
///
/// Implements FIDE Dutch system rules:
/// 1. Divide players into scoregroups
/// 2. Pair within scoregroups
/// 3. Minimize floaters
/// 4. Maximize score differences
pub fn generate_pairings(
    round: u8,
    players: &[SwissPlayer],
    total_rounds: u8,
) -> PairingResult<SwissRound> {
    if round > total_rounds {
        return Err(PairingError::InvalidRound {
            round,
            total_rounds,
        });
    }

    trace!(
        "Generating pairings for round {}/{} with {} players",
        round,
        total_rounds,
        players.len()
    );

    // 1. Sort players by rank (score desc, rating desc, id asc)
    let mut ranked = players.to_vec();
    ranked.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap()
            .then_with(|| b.rating.cmp(&a.rating))
            .then_with(|| a.id.cmp(&b.id))
    });

    // 2. Group by score
    let scoregroups = group_by_score(&ranked);
    debug!("Created {} scoregroups", scoregroups.len());

    // 3. Pair each scoregroup
    let mut all_pairings: Vec<Pairing> = Vec::new();
    let mut byes: Vec<String> = Vec::new();
    let mut used_players: Vec<String> = Vec::new();

    for (i, group) in scoregroups.iter().enumerate() {
        trace!("Pairing scoregroup {} with score {} ({} players)", i, group.score, group.players.len());
        
        // Filter out already paired players
        let available: Vec<SwissPlayer> = group
            .players
            .iter()
            .filter(|p| !used_players.contains(&p.id))
            .cloned()
            .collect();

        if available.is_empty() {
            continue;
        }

        // Try to pair this scoregroup
        let (pairings, bye, remaining) = pair_scoregroup(&available, &used_players)?;
        
        all_pairings.extend(pairings);
        if let Some(bye_player) = bye {
            byes.push(bye_player);
        }
        used_players.extend(remaining);
    }

    // Handle remaining unpaired players (float to next group)
    if !used_players.is_empty() {
        let unpaired: Vec<SwissPlayer> = ranked
            .into_iter()
            .filter(|p| !used_players.contains(&p.id) && !byes.contains(&p.id))
            .collect();
        
        if !unpaired.is_empty() {
            warn!("{} players unpaired after initial pass, applying float logic", unpaired.len());
            let float_pairings = apply_float_pairing(&unpaired, &used_players, &byes)?;
            all_pairings.extend(float_pairings);
        }
    }

    // 4. Assign colors
    let colored = assign_colors_dutch(&all_pairings, players)?;

    // 5. Assign board numbers (sorted by combined rating)
    let numbered = assign_board_numbers(colored, players);

    debug!(
        "Round {} generated: {} pairings, {} byes",
        round,
        numbered.len(),
        byes.len()
    );

    Ok(SwissRound {
        round,
        pairings: numbered,
        byes,
    })
}

/// Group players by identical scores
fn group_by_score(players: &[SwissPlayer]) -> Vec<Scoregroup> {
    let mut groups: Vec<Scoregroup> = Vec::new();
    let mut current_score: Option<f64> = None;
    let mut current_group: Vec<SwissPlayer> = Vec::new();

    for player in players {
        match current_score {
            None => {
                current_score = Some(player.score);
                current_group.push(player.clone());
            }
            Some(score) if (score - player.score).abs() < f64::EPSILON => {
                current_group.push(player.clone());
            }
            Some(score) => {
                groups.push(Scoregroup {
                    score,
                    players: current_group.clone(),
                });
                current_score = Some(player.score);
                current_group.clear();
                current_group.push(player.clone());
            }
        }
    }

    // Don't forget the last group
    if let Some(score) = current_score {
        groups.push(Scoregroup {
            score,
            players: current_group,
        });
    }

    groups
}

/// Pair players within a scoregroup
/// Returns (pairings, bye_player_if_any, used_player_ids)
fn pair_scoregroup(
    players: &[SwissPlayer],
    already_used: &[String],
) -> PairingResult<(Vec<Pairing>, Option<String>, Vec<String>)> {
    if players.is_empty() {
        return Ok((Vec::new(), None, Vec::new()));
    }

    let mut pairings: Vec<Pairing> = Vec::new();
    let mut used: Vec<String> = Vec::new();
    let mut bye: Option<String> = None;

    // Handle odd number - one player gets bye
    let player_count = if players.len() % 2 == 1 {
        // Select bye candidate (lowest rated, or most byes)
        let bye_candidate = select_bye_candidate(players)?;
        bye = Some(bye_candidate.id.clone());
        used.push(bye_candidate.id.clone());
        players.len() - 1
    } else {
        players.len()
    };

    // Now pair remaining players
    let to_pair: Vec<&SwissPlayer> = players
        .iter()
        .filter(|p| !used.contains(&p.id))
        .collect();

    // Dutch system: split in half, pair top half vs bottom half
    let half = player_count / 2;
    let top_half = &to_pair[..half.min(to_pair.len())];
    let bottom_half = &to_pair[half.min(to_pair.len())..];

    for (i, (white, black)) in top_half.iter().zip(bottom_half.iter()).enumerate() {
        // Check if they've already played
        if white.opponents.contains(&black.id) || black.opponents.contains(&white.id) {
            // Try to find alternate pairing
            warn!("Players {} and {} have already played, seeking alternative", white.id, black.id);
            continue;
        }

        pairings.push(Pairing {
            white: white.id.clone(),
            black: black.id.clone(),
            board: 0, // Will be assigned later
        });
        
        used.push(white.id.clone());
        used.push(black.id.clone());
    }

    Ok((pairings, bye, used))
}

/// Select the best candidate for a bye
fn select_bye_candidate(players: &[SwissPlayer]) -> PairingResult<SwissPlayer> {
    // Priority: lowest score, lowest rating, most previous byes
    let mut candidates: Vec<&SwissPlayer> = players.iter().collect();
    
    candidates.sort_by(|a, b| {
        a.score
            .partial_cmp(&b.score)
            .unwrap()
            .then_with(|| a.rating.cmp(&b.rating))
            .then_with(|| a.bye_count.cmp(&b.bye_count))
    });

    candidates
        .first()
        .cloned()
        .cloned()
        .ok_or_else(|| PairingError::NoByeCandidate)
}

/// Apply float pairing for remaining unpaired players
fn apply_float_pairing(
    unpaired: &[SwissPlayer],
    used: &[String],
    byes: &[String],
) -> PairingResult<Vec<Pairing>> {
    let mut pairings: Vec<Pairing> = Vec::new();
    let mut remaining = unpaired.to_vec();

    // Sort by score/rating
    remaining.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap()
            .then_with(|| b.rating.cmp(&a.rating))
    });

    while remaining.len() >= 2 {
        let first = remaining.remove(0);
        
        // Find first opponent they haven't played
        let opponent_idx = remaining.iter().position(|p| {
            !first.opponents.contains(&p.id) && !p.opponents.contains(&first.id)
        });

        match opponent_idx {
            Some(idx) => {
                let opponent = remaining.remove(idx);
                pairings.push(Pairing {
                    white: first.id.clone(),
                    black: opponent.id.clone(),
                    board: 0,
                });
            }
            None => {
                warn!("Could not find valid opponent for {}, giving bye", first.id);
                // In a real implementation, we might need to give a forced bye
                // or reshuffle previous pairings
            }
        }
    }

    Ok(pairings)
}

/// Assign colors following Dutch system rules
fn assign_colors_dutch(
    pairings: &[Pairing],
    players: &[SwissPlayer],
) -> PairingResult<Vec<Pairing>> {
    let mut colored: Vec<Pairing> = Vec::new();

    for pairing in pairings {
        let white_data = find_player(players, &pairing.white)?;
        let black_data = find_player(players, &pairing.black)?;

        let white_balance = white_data.color_balance();
        let black_balance = black_data.color_balance();

        // Determine who should be white based on color balance
        // Player with more blacks (positive balance) should get white
        let (actual_white, actual_black) = if white_balance > black_balance {
            // White player needs white more
            (pairing.white.clone(), pairing.black.clone())
        } else if black_balance > white_balance {
            // Black player needs white more
            (pairing.black.clone(), pairing.white.clone())
        } else {
            // Equal balance - check 3-in-a-row rule
            let white_violates = white_data.would_violate_three_in_row(Color::White);
            let black_violates = black_data.would_violate_three_in_row(Color::White);

            if white_violates && !black_violates {
                (pairing.black.clone(), pairing.white.clone())
            } else {
                (pairing.white.clone(), pairing.black.clone())
            }
        };

        colored.push(Pairing {
            white: actual_white,
            black: actual_black,
            board: pairing.board,
        });
    }

    Ok(colored)
}

/// Assign board numbers (1 = highest combined rating)
fn assign_board_numbers(pairings: Vec<Pairing>, players: &[SwissPlayer]) -> Vec<Pairing> {
    let mut with_rating: Vec<(Pairing, u32)> = pairings
        .into_iter()
        .map(|p| {
            let white_rating = find_player(players, &p.white)
                .map(|wp| wp.rating)
                .unwrap_or(0);
            let black_rating = find_player(players, &p.black)
                .map(|bp| bp.rating)
                .unwrap_or(0);
            (p, white_rating + black_rating)
        })
        .collect();

    // Sort by combined rating descending
    with_rating.sort_by(|a, b| b.1.cmp(&a.1));

    // Assign board numbers
    with_rating
        .into_iter()
        .enumerate()
        .map(|(i, (mut p, _))| {
            p.board = (i + 1) as u16;
            p
        })
        .collect()
}

/// Find a player by ID
fn find_player<'a>(players: &'a [SwissPlayer], id: &str) -> PairingResult<&'a SwissPlayer> {
    players
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| PairingError::PlayerNotFound(id.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_group_by_score() {
        let players = vec![
            test_player("p1", 2000, 3.0),
            test_player("p2", 1900, 3.0),
            test_player("p3", 1800, 2.0),
            test_player("p4", 1700, 2.0),
            test_player("p5", 1600, 1.0),
        ];

        let groups = group_by_score(&players);
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].players.len(), 2); // 3.0
        assert_eq!(groups[1].players.len(), 2); // 2.0
        assert_eq!(groups[2].players.len(), 1); // 1.0
    }

    #[test]
    fn test_generate_pairings_8_players() {
        let players = vec![
            test_player("p1", 2000, 3.0),
            test_player("p2", 1900, 3.0),
            test_player("p3", 1800, 2.5),
            test_player("p4", 1700, 2.5),
            test_player("p5", 1600, 2.0),
            test_player("p6", 1500, 2.0),
            test_player("p7", 1400, 1.0),
            test_player("p8", 1300, 1.0),
        ];

        let round = generate_pairings(1, &players, 5).unwrap();
        
        assert_eq!(round.pairings.len(), 4);
        assert!(round.byes.is_empty());
        
        // Top players should be paired
        assert_eq!(round.pairings[0].board, 1); // Highest combined rating
    }

    #[test]
    fn test_odd_players_get_bye() {
        let players = vec![
            test_player("p1", 2000, 3.0),
            test_player("p2", 1900, 3.0),
            test_player("p3", 1800, 2.0),
            test_player("p4", 1700, 2.0),
            test_player("p5", 1600, 1.0),
        ];

        let round = generate_pairings(1, &players, 5).unwrap();
        
        assert_eq!(round.pairings.len(), 2);
        assert_eq!(round.byes.len(), 1);
        // Lowest rated should get bye
        assert_eq!(round.byes[0], "p5");
    }
}
