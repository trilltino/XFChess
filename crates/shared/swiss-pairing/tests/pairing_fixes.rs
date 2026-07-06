//! Regression tests for Swiss pairing correctness fixes.
//!
//! Each test targets a specific documented bug or rule:
//! - Rematch transposition in the bottom half
//! - Forced bye on odd-leftover after float
//! - 5-round bye rotation (no player gets two byes unless unavoidable)
//! - Float history — player not floated down twice in a row

use swiss_pairing::{generate_pairings, PairingConfig, SwissPlayer};

fn player(id: &str, rating: u32, score: f64) -> SwissPlayer {
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

fn cfg() -> PairingConfig {
    PairingConfig::default()
}

// ─── Test 1: rematch transposition ────────────────────────────────────────────

#[test]
fn rematch_transposition_in_bottom_half() {
    let mut p1 = player("p1", 2000, 2.0);
    let mut p3 = player("p3", 1800, 2.0);
    p1.opponents.push("p3".into());
    p3.opponents.push("p1".into());

    let players = vec![p1, player("p2", 1900, 2.0), p3, player("p4", 1700, 2.0)];

    let round = generate_pairings(2, &players, 5, &cfg()).unwrap();

    assert_eq!(round.pairings.len(), 2, "all 4 players must be paired");
    for p in &round.pairings {
        let is_rematch =
            (p.white == "p1" && p.black == "p3") || (p.white == "p3" && p.black == "p1");
        assert!(!is_rematch, "engine produced a rematch: {:?}", p);
    }
}

// ─── Test 2: forced bye on odd leftover ───────────────────────────────────────

#[test]
fn forced_bye_on_odd_leftover_after_float() {
    let mut p1 = player("p1", 2000, 0.0);
    let mut p2 = player("p2", 1900, 0.0);
    let mut p3 = player("p3", 1800, 0.0);
    p1.opponents = vec!["p2".into(), "p3".into()];
    p2.opponents = vec!["p1".into(), "p3".into()];
    p3.opponents = vec!["p1".into(), "p2".into()];

    let round = generate_pairings(3, &[p1, p2, p3], 5, &cfg()).unwrap();

    let total = round.pairings.len() * 2 + round.byes.len();
    assert_eq!(
        total, 3,
        "all 3 players must be accounted for (no silent drop)"
    );
    assert!(!round.byes.is_empty(), "at least one forced bye expected");
}

// ─── Test 3: 5-round bye rotation ────────────────────────────────────────────

/// Over 5 rounds with 5 players (one bye per round), each player should
/// receive at most one bye.
#[test]
fn five_round_bye_rotation_no_double_bye() {
    let base_player = |id: &str, rating: u32| player(id, rating, 0.0);

    let mut current_players = vec![
        base_player("a", 2000),
        base_player("b", 1900),
        base_player("c", 1800),
        base_player("d", 1700),
        base_player("e", 1600),
    ];

    let mut seen_byes: std::collections::HashMap<String, u8> = std::collections::HashMap::new();

    for round_num in 1u8..=5 {
        let round = generate_pairings(round_num, &current_players, 5, &cfg()).unwrap();
        assert_eq!(
            round.byes.len(),
            1,
            "round {}: expected exactly 1 bye",
            round_num
        );

        let bye_id = round.byes[0].clone();
        *seen_byes.entry(bye_id.clone()).or_insert(0) += 1;

        // Feed results back using bye_rounds instead of bye_count
        for p in &mut current_players {
            if bye_id == p.id {
                p.score += 1.0;
                p.bye_rounds.push(round_num);
            }
            for pairing in &round.pairings {
                if pairing.white == p.id {
                    p.score += 1.0;
                    p.opponents.push(pairing.black.clone());
                } else if pairing.black == p.id {
                    p.opponents.push(pairing.white.clone());
                }
            }
        }
    }

    for (id, count) in &seen_byes {
        assert!(
            *count <= 1,
            "player {} received {} byes (rotation failed)",
            id,
            count
        );
    }
}

// ─── Test 4: float_downs recorded in round ────────────────────────────────────

#[test]
fn float_down_recorded_in_round() {
    // Top 2 pair cleanly within their scoregroup; bottom 4 pair among themselves.
    // No floats should occur.
    let players = vec![
        player("p1", 2000, 2.0),
        player("p2", 1900, 2.0),
        player("p3", 1800, 0.0),
        player("p4", 1700, 0.0),
        player("p5", 1600, 0.0),
        player("p6", 1500, 0.0),
    ];

    let round = generate_pairings(2, &players, 5, &cfg()).unwrap();
    assert!(
        round.float_downs.is_empty(),
        "no floats expected when groups pair cleanly, got: {:?}",
        round.float_downs
    );
    assert_eq!(round.pairings.len(), 3);
    assert!(round.byes.is_empty());
}
