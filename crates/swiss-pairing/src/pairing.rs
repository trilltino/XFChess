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
    let unpaired: Vec<SwissPlayer> = ranked
        .iter()
        .filter(|p| !used_players.contains(&p.id) && !byes.contains(&p.id))
        .cloned()
        .collect();

    // Track which players float down this round so subsequent rounds can avoid
    // floating the same player down twice in a row.
    let float_downs: Vec<String> = unpaired.iter().map(|p| p.id.clone()).collect();

    // Track float-ups: players from a lower scoregroup who were paired into
    // a higher one. Detected by checking if a player's scoregroup differs
    // from their pairing partner's.
    let mut float_ups: Vec<String> = Vec::new();
    for pairing in &all_pairings {
        let white_score = ranked.iter().find(|p| p.id == pairing.white).map(|p| p.score);
        let black_score = ranked.iter().find(|p| p.id == pairing.black).map(|p| p.score);
        if let (Some(ws), Some(bs)) = (white_score, black_score) {
            if (bs - ws).abs() > f64::EPSILON && ws < bs {
                // White player has lower score → floated up
                if !float_ups.contains(&pairing.white) {
                    float_ups.push(pairing.white.clone());
                }
            } else if (ws - bs).abs() > f64::EPSILON && bs < ws {
                // Black player has lower score → floated up
                if !float_ups.contains(&pairing.black) {
                    float_ups.push(pairing.black.clone());
                }
            }
        }
    }

    if !unpaired.is_empty() {
        warn!(
            "{} players unpaired after initial pass, applying float logic",
            unpaired.len()
        );
        let (float_pairings, float_byes) = apply_float_pairing(&unpaired)?;
        all_pairings.extend(float_pairings);
        byes.extend(float_byes);
    }

    // 4. Assign colors
    let colored = assign_colors_dutch(&all_pairings, players)?;

    // 5. Assign board numbers (sorted by combined rating)
    let numbered = assign_board_numbers(colored, players);

    debug!(
        "Round {} generated: {} pairings, {} byes, {} float-downs",
        round,
        numbered.len(),
        byes.len(),
        float_downs.len()
    );

    Ok(SwissRound {
        round,
        pairings: numbered,
        byes,
        float_downs,
        float_ups,
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

    // Handle odd number - one player gets bye.
    // A 1-player group should float to the next scoregroup, not receive a bye here.
    let player_count = if players.len() % 2 == 1 && players.len() > 1 {
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

    // Mutable bottom-half so we can swap when the natural pairing was a rematch.
    let mut bottom: Vec<&SwissPlayer> = bottom_half.to_vec();

    for white in top_half.iter() {
        // First try the current head of bottom_half; if that was a rematch,
        // walk the rest of bottom to find a non-rematch partner and swap.
        if bottom.is_empty() {
            break;
        }

        let mut chosen: Option<usize> = None;
        for (j, candidate) in bottom.iter().enumerate() {
            let rematch = white.opponents.contains(&candidate.id)
                || candidate.opponents.contains(&white.id);
            if !rematch {
                chosen = Some(j);
                break;
            }
        }

        match chosen {
            Some(j) => {
                let black = bottom.remove(j);
                pairings.push(Pairing {
                    white: white.id.clone(),
                    black: black.id.clone(),
                    board: 0, // assigned later
                });
                used.push(white.id.clone());
                used.push(black.id.clone());
            }
            None => {
                // All remaining bottom-half players are rematches with this top
                // player; leave this top player unpaired — `apply_float_pairing`
                // will float them into the next scoregroup.
                warn!(
                    "Player {} has played every remaining opponent in scoregroup; floating",
                    white.id
                );
            }
        }
    }

    Ok((pairings, bye, used))
}

/// Select the best candidate for a bye.
///
/// FIDE rule: a player should receive at most one bye per tournament. We
/// prefer candidates who have never had a bye; fall back to lowest bye_count
/// if everyone in the pool has already had one (rare, only in tiny fields).
///
/// Tie-break order:
/// 1. fewest previous byes,
/// 2. lowest current score,
/// 3. lowest rating,
/// 4. player id (stable).
///
/// Reference: FIDE Handbook C.04.2 — <https://handbook.fide.com/chapter/C0402>
fn select_bye_candidate(players: &[SwissPlayer]) -> PairingResult<SwissPlayer> {
    let mut candidates: Vec<&SwissPlayer> = players.iter().collect();

    candidates.sort_by(|a, b| {
        a.bye_count
            .cmp(&b.bye_count)
            .then_with(|| a.score.partial_cmp(&b.score).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| a.rating.cmp(&b.rating))
            .then_with(|| a.id.cmp(&b.id))
    });

    candidates
        .first()
        .cloned()
        .cloned()
        .ok_or(PairingError::NoByeCandidate)
}

/// Apply float pairing for remaining unpaired players.
///
/// Returns `(pairings, forced_byes)`.
///
/// When the player count is odd, one player is pre-selected as the bye
/// recipient using `select_bye_candidate` (FIDE rules: fewest prior byes,
/// then lowest score, then lowest rating). The remaining even-count pool is
/// then paired with rematch avoidance; if no non-rematch partner exists the
/// engine accepts the least-bad rematch rather than issuing a forced bye.
fn apply_float_pairing(
    unpaired: &[SwissPlayer],
) -> PairingResult<(Vec<Pairing>, Vec<String>)> {
    let mut pairings: Vec<Pairing> = Vec::new();
    let mut forced_byes: Vec<String> = Vec::new();
    let mut remaining = unpaired.to_vec();

    remaining.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.rating.cmp(&a.rating))
            .then_with(|| a.id.cmp(&b.id))
    });

    // Pre-select the bye recipient (FIDE rule: fewest prior byes first).
    if remaining.len() % 2 == 1 {
        let bye = select_bye_candidate(&remaining)?;
        remaining.retain(|p| p.id != bye.id);
        forced_byes.push(bye.id);
    }

    // Pair the remaining even-count pool.
    while remaining.len() >= 2 {
        let first = remaining.remove(0);

        // Prefer a non-rematch partner; fall back to the first available
        // (accepting a rematch) rather than giving another forced bye.
        let no_clean = remaining
            .iter()
            .all(|p| first.opponents.contains(&p.id) || p.opponents.contains(&first.id));
        let partner_idx = if no_clean {
            warn!("No non-rematch opponent for {}, accepting rematch", first.id);
            0
        } else {
            remaining
                .iter()
                .position(|p| {
                    !first.opponents.contains(&p.id) && !p.opponents.contains(&first.id)
                })
                .unwrap_or(0)
        };

        let partner = remaining.remove(partner_idx);
        pairings.push(Pairing {
            white: first.id.clone(),
            black: partner.id.clone(),
            board: 0,
        });
    }

    Ok((pairings, forced_byes))
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

    #[test]
    fn bye_rotation_prefers_player_with_no_prior_bye() {
        // p5 has already had a bye; p4 has none. Even though p4 is higher
        // rated than p5, p4 should still NOT be the bye pick because
        // bye_count is the primary tie-break.
        let mut p4 = test_player("p4", 1700, 1.0);
        let mut p5 = test_player("p5", 1600, 1.0);
        p5.bye_count = 1;
        // Tie them on score so the bye decision is driven by bye_count/rating.
        p4.score = 1.0;
        p5.score = 1.0;

        let players = vec![
            test_player("p1", 2000, 2.0),
            test_player("p2", 1900, 2.0),
            test_player("p3", 1800, 1.0),
            p4,
            p5,
        ];

        let round = generate_pairings(2, &players, 5).unwrap();

        assert_eq!(round.byes.len(), 1);
        assert_ne!(round.byes[0], "p5", "p5 already had a bye; should not receive another");
    }

    #[test]
    fn rematch_is_avoided_by_swap_in_bottom_half() {
        // 4-player scoregroup where the natural top-vs-bottom pairing
        // (p1 vs p3, p2 vs p4) is a rematch: p1-p3 already played.
        // Engine must swap to pair (p1 vs p4) and (p2 vs p3) instead.
        let mut p1 = test_player("p1", 2000, 1.0);
        let mut p3 = test_player("p3", 1800, 1.0);
        p1.opponents.push("p3".into());
        p3.opponents.push("p1".into());

        let players = vec![
            p1,
            test_player("p2", 1900, 1.0),
            p3,
            test_player("p4", 1700, 1.0),
        ];

        let round = generate_pairings(2, &players, 5).unwrap();

        assert_eq!(round.pairings.len(), 2);
        for p in &round.pairings {
            let rematch = (p.white == "p1" && p.black == "p3")
                || (p.white == "p3" && p.black == "p1");
            assert!(!rematch, "engine produced a rematch: {:?}", p);
        }
    }

    #[test]
    fn forced_bye_emitted_when_no_valid_opponent_after_float() {
        // Three-player odd field where every remaining pair has played.
        // We should see at least one bye rather than silently dropping a player.
        let mut p1 = test_player("p1", 2000, 0.0);
        let mut p2 = test_player("p2", 1900, 0.0);
        let mut p3 = test_player("p3", 1800, 0.0);
        p1.opponents = vec!["p2".into(), "p3".into()];
        p2.opponents = vec!["p1".into(), "p3".into()];
        p3.opponents = vec!["p1".into(), "p2".into()];

        let round = generate_pairings(3, &[p1, p2, p3], 5).unwrap();

        // Odd count => at least one bye; no silent player drop.
        let total_accounted = round.pairings.len() * 2 + round.byes.len();
        assert_eq!(total_accounted, 3, "every player must be accounted for");
        assert!(!round.byes.is_empty());
    }
}
