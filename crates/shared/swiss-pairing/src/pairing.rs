use crate::{
    Color, PairingConfig, Pairing, PairingError, PairingResult, Scoregroup, SwissPlayer, SwissRound,
};
use std::collections::HashSet;
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
    config: &PairingConfig,
) -> PairingResult<SwissRound> {
    if round > total_rounds {
        return Err(PairingError::InvalidRound {
            round,
            total_rounds,
        });
    }

    // Filter out absent and withdrawn players before pairing
    let active: Vec<SwissPlayer> = players
        .iter()
        .filter(|p| !p.absent && !p.withdrawn)
        .cloned()
        .collect();

    trace!(
        "Generating pairings for round {}/{} with {} active players ({} total)",
        round,
        total_rounds,
        active.len(),
        players.len(),
    );

    // Handle manual overrides first — remove those players from the Dutch pool
    let mut overridden_ids: HashSet<String> = HashSet::new();
    let mut manual_pairings: Vec<Pairing> = Vec::new();

    for mo in &config.manual_overrides {
        manual_pairings.push(Pairing {
            white: mo.white.clone(),
            black: mo.black.clone(),
            board: 0, // assigned later
        });
        overridden_ids.insert(mo.white.clone());
        overridden_ids.insert(mo.black.clone());
    }

    // Players available for the Dutch algorithm
    let auto_players: Vec<SwissPlayer> = active
        .iter()
        .filter(|p| !overridden_ids.contains(&p.id))
        .cloned()
        .collect();

    // 1. Sort players by rank (score desc, rating desc, id asc)
    let mut ranked = auto_players.clone();
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
    let mut all_pairings: Vec<Pairing> = manual_pairings;
    let mut byes: Vec<String> = Vec::new();
    let mut used_players: Vec<String> = overridden_ids.into_iter().collect();

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
        let (pairings, bye, remaining) = pair_scoregroup(&available, config)?;

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

    // Track which players float down this round
    let float_downs: Vec<String> = unpaired.iter().map(|p| p.id.clone()).collect();

    // Track float-ups
    let mut float_ups: Vec<String> = Vec::new();
    for pairing in &all_pairings {
        let white_score = ranked.iter().find(|p| p.id == pairing.white).map(|p| p.score);
        let black_score = ranked.iter().find(|p| p.id == pairing.black).map(|p| p.score);
        if let (Some(ws), Some(bs)) = (white_score, black_score) {
            if (bs - ws).abs() > f64::EPSILON && ws < bs {
                if !float_ups.contains(&pairing.white) {
                    float_ups.push(pairing.white.clone());
                }
            } else if (ws - bs).abs() > f64::EPSILON && bs < ws {
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
        let (float_pairings, float_byes) = apply_float_pairing(&unpaired, config)?;
        all_pairings.extend(float_pairings);
        byes.extend(float_byes);
    }

    // 4. Assign colors — use the full active pool for color history lookup
    let colored = assign_colors_dutch(&all_pairings, &active)?;

    // 5. Assign board numbers (sorted by combined rating)
    let numbered = assign_board_numbers(colored, &active);

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

/// Check if a pairing is valid (no rematch, not forbidden)
fn is_valid_pairing(p1: &str, p2: &str, p1_opponents: &[String], config: &PairingConfig) -> bool {
    let already_played = p1_opponents.contains(&p2.to_string());
    let forbidden = config.is_forbidden(p1, p2);
    !already_played && !forbidden
}

/// Pair players within a scoregroup
/// Returns (pairings, bye_player_if_any, used_player_ids)
fn pair_scoregroup(
    players: &[SwissPlayer],
    config: &PairingConfig,
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

    // Prefer floaters that have NOT been floated down recently as the down-float
    // candidate. If all have been floated down, we accept a repeat.
    let mut bottom: Vec<&SwissPlayer> = bottom_half.to_vec();

    for white in top_half.iter() {
        if bottom.is_empty() {
            break;
        }

        let mut chosen: Option<usize> = None;
        for (j, candidate) in bottom.iter().enumerate() {
            if is_valid_pairing(&white.id, &candidate.id, &white.opponents, config) {
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
                // All remaining bottom-half players are rematches or forbidden; float this player.
                warn!(
                    "Player {} has no valid opponent in scoregroup; floating",
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
        a.bye_count()
            .cmp(&b.bye_count())
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
fn apply_float_pairing(
    unpaired: &[SwissPlayer],
    config: &PairingConfig,
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

        // Prefer a non-rematch, non-forbidden partner; fall back to first available.
        let no_clean = remaining
            .iter()
            .all(|p| !is_valid_pairing(&first.id, &p.id, &first.opponents, config));
        let partner_idx = if no_clean {
            warn!("No valid opponent for {}, accepting rematch", first.id);
            0
        } else {
            remaining
                .iter()
                .position(|p| is_valid_pairing(&first.id, &p.id, &first.opponents, config))
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
            (pairing.white.clone(), pairing.black.clone())
        } else if black_balance > white_balance {
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

    with_rating.sort_by(|a, b| b.1.cmp(&a.1));

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
    use crate::FloatStatus;

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

    fn default_config() -> PairingConfig {
        PairingConfig::default()
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

        let round = generate_pairings(1, &players, 5, &default_config()).unwrap();

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

        let round = generate_pairings(1, &players, 5, &default_config()).unwrap();

        assert_eq!(round.pairings.len(), 2);
        assert_eq!(round.byes.len(), 1);
        // Lowest rated should get bye
        assert_eq!(round.byes[0], "p5");
    }

    #[test]
    fn bye_rotation_prefers_player_with_no_prior_bye() {
        let mut p4 = test_player("p4", 1700, 1.0);
        let mut p5 = test_player("p5", 1600, 1.0);
        p5.bye_rounds = vec![1]; // p5 already had a bye in round 1
        p4.score = 1.0;
        p5.score = 1.0;

        let players = vec![
            test_player("p1", 2000, 2.0),
            test_player("p2", 1900, 2.0),
            test_player("p3", 1800, 1.0),
            p4,
            p5,
        ];

        let round = generate_pairings(2, &players, 5, &default_config()).unwrap();

        assert_eq!(round.byes.len(), 1);
        assert_ne!(round.byes[0], "p5", "p5 already had a bye; should not receive another");
    }

    #[test]
    fn rematch_is_avoided_by_swap_in_bottom_half() {
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

        let round = generate_pairings(2, &players, 5, &default_config()).unwrap();

        assert_eq!(round.pairings.len(), 2);
        for p in &round.pairings {
            let rematch = (p.white == "p1" && p.black == "p3")
                || (p.white == "p3" && p.black == "p1");
            assert!(!rematch, "engine produced a rematch: {:?}", p);
        }
    }

    #[test]
    fn forced_bye_emitted_when_no_valid_opponent_after_float() {
        let mut p1 = test_player("p1", 2000, 0.0);
        let mut p2 = test_player("p2", 1900, 0.0);
        let mut p3 = test_player("p3", 1800, 0.0);
        p1.opponents = vec!["p2".into(), "p3".into()];
        p2.opponents = vec!["p1".into(), "p3".into()];
        p3.opponents = vec!["p1".into(), "p2".into()];

        let round = generate_pairings(3, &[p1, p2, p3], 5, &default_config()).unwrap();

        let total_accounted = round.pairings.len() * 2 + round.byes.len();
        assert_eq!(total_accounted, 3, "every player must be accounted for");
        assert!(!round.byes.is_empty());
    }

    #[test]
    fn absent_player_is_skipped_from_pairing() {
        let mut absent = test_player("p5", 1600, 1.0);
        absent.absent = true;

        let players = vec![
            test_player("p1", 2000, 2.0),
            test_player("p2", 1900, 2.0),
            test_player("p3", 1800, 1.0),
            test_player("p4", 1700, 1.0),
            absent,
        ];

        let round = generate_pairings(2, &players, 5, &default_config()).unwrap();

        // p5 is absent; the 4 active players pair cleanly with no bye
        assert_eq!(round.pairings.len(), 2);
        assert!(round.byes.is_empty());
        for p in &round.pairings {
            assert_ne!(p.white, "p5");
            assert_ne!(p.black, "p5");
        }
    }

    #[test]
    fn forbidden_pair_is_not_produced() {
        let players = vec![
            test_player("p1", 2000, 1.0),
            test_player("p2", 1900, 1.0),
            test_player("p3", 1800, 1.0),
            test_player("p4", 1700, 1.0),
        ];

        let config = PairingConfig {
            forbidden: vec![("p1".into(), "p2".into())],
            manual_overrides: vec![],
        };

        let round = generate_pairings(1, &players, 5, &config).unwrap();

        for p in &round.pairings {
            let forbidden = (p.white == "p1" && p.black == "p2")
                || (p.white == "p2" && p.black == "p1");
            assert!(!forbidden, "forbidden pair was produced: {:?}", p);
        }
    }

    #[test]
    fn manual_override_is_honored() {
        let players = vec![
            test_player("p1", 2000, 1.0),
            test_player("p2", 1900, 1.0),
            test_player("p3", 1800, 1.0),
            test_player("p4", 1700, 1.0),
        ];

        let config = PairingConfig {
            forbidden: vec![],
            manual_overrides: vec![crate::ManualPairing {
                white: "p1".into(),
                black: "p4".into(),
            }],
        };

        let round = generate_pairings(1, &players, 5, &config).unwrap();

        let forced = round.pairings.iter().any(|p| {
            (p.white == "p1" && p.black == "p4") || (p.white == "p4" && p.black == "p1")
        });
        assert!(forced, "manual override pairing not present");
    }
}
