use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use tower::ServiceExt; // for `oneshot`

use backend::infrastructure::{build_app_router, initialize_pools, run_migrations};
use backend::signing::orchestrator::spawn_orchestrator;
use backend::signing::storage::tournament::{
    GameOutcome, TournamentFormat, TournamentRecord, TournamentStore,
};
use backend::signing::OrchestratorEvent;
use backend::signing::{AppState, SigningConfig};

fn unique_db_url(tag: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("sqlite:file:xfchess_swiss_e2e_{tag}_{n}_{nanos}?mode=memory&cache=shared")
}

fn test_config() -> SigningConfig {
    SigningConfig {
        port: 0,
        solana_rpc_url: "http://127.0.0.1:9".into(),
        solana_mainnet_rpc_url: None,
        er_rpc_url: "http://127.0.0.1:9".into(),
        magic_router_rpc_url: "http://127.0.0.1:9".into(),
        program_id: "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU".into(),
        jwt_secret: "test-secret-not-for-production".into(),
        identity_encryption_key: "0".repeat(64),
        identity_salt: "0".repeat(64),
        fee_payer_keys: vec![],
        vps_authority_key: None,
        kyc_authority_key: None,
        link_authority_key: None,
        treasury_authority_pubkey: "9jpjASzudVvpbgw5G7zCf7o6EvCw4ejRVcEN1aBLq4Kd".to_string(),
        admin_token: Some("test-admin-token".into()),
        tournament_fee_recipient: "uLgR6Nx4KqQobj6e2mQUPeWQpMUauDRc2oz6wZg3Y6C".into(),
        usdc_mint_pubkey: "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU".into(),
        lichess_client_id: String::new(),
        allowed_origins: vec![],
    }
}

struct TestApp {
    state: AppState,
}

impl TestApp {
    fn router(&self) -> Router {
        build_app_router(self.state.clone()).with_state(self.state.clone())
    }

    async fn get(&self, uri: &str) -> (StatusCode, Value) {
        let req = Request::builder()
            .uri(uri)
            .method("GET")
            .body(Body::empty())
            .unwrap();
        self.send(req).await
    }

    async fn post_json(&self, uri: &str, body: &Value) -> (StatusCode, Value) {
        let req = Request::builder()
            .uri(uri)
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(body).unwrap()))
            .unwrap();
        self.send(req).await
    }

    async fn post_json_admin(&self, uri: &str, body: &Value) -> (StatusCode, Value) {
        let req = Request::builder()
            .uri(uri)
            .method("POST")
            .header("content-type", "application/json")
            .header("X-API-Key", "dev")
            .body(Body::from(serde_json::to_vec(body).unwrap()))
            .unwrap();
        self.send(req).await
    }

    async fn send(&self, req: Request<Body>) -> (StatusCode, Value) {
        let resp = self.router().oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, value)
    }
}

async fn spawn_app() -> TestApp {
    let pools = initialize_pools(&unique_db_url("session"), &unique_db_url("vault"))
        .await
        .expect("init pools");
    run_migrations(&pools).await.expect("run migrations");

    let tournament_store = TournamentStore::new(pools.session_pool.clone()).await;

    let state = AppState::new(
        test_config(),
        pools.session_pool.clone(),
        pools.vault_pool.clone(),
        Arc::new(tournament_store),
    );

    TestApp { state }
}

#[tokio::test]
async fn swiss_tournament_full_lifecycle_via_http() {
    let app = spawn_app().await;
    let tournament_id = 9001_u64;

    let players: [(&str, u32); 8] = [
        ("gm_wallet", 2600),
        ("m1_wallet", 2400),
        ("m2_wallet", 2300),
        ("e1_wallet", 2100),
        ("e2_wallet", 2000),
        ("c1_wallet", 1600),
        ("c2_wallet", 1500),
        ("novice_wallet", 1200),
    ];

    // Seed the tournament directly into the store (see module doc: this
    // skips the on-chain-coupled `POST /admin/tournament/create`).
    let record = TournamentRecord::with_config(
        tournament_id,
        "Test Swiss Tournament".to_string(),
        1_000_000,
        0,
        8,
        [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0],
        TournamentFormat::Swiss { rounds: 5 },
        None,
        None,
        Some(8),
        None,
        false,
    );
    app.state.tournament_store.create(record).await;

    seed_players(&app, tournament_id, players).await;

    // Initialize Swiss: seeds by ELO, marks Active, starts round 1.
    let (status, body) = app
        .post_json_admin(
            &format!("/admin/tournament/{tournament_id}/initialize-swiss"),
            &json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "initialize-swiss: {body}");

    let (status, tournament) = app.get(&format!("/tournament/{tournament_id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tournament["status"], json!("Active"));
    assert_eq!(tournament["players"].as_array().unwrap().len(), 8);

    let (status, round) = app
        .get(&format!("/tournament/{tournament_id}/current-round"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(round["round"], json!(1));
    assert_eq!(round["total_rounds"], json!(5));
    assert_eq!(round["is_active"], json!(true));

    let (status, my_match) = app
        .get(&format!(
            "/tournament/{tournament_id}/my-match?player=gm_wallet"
        ))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(my_match["found"], json!(true));
    assert_eq!(my_match["round"], json!(0), "0-based like bracket rounds");
    assert!(my_match["board"].is_number());

    // Round 1 pairings: 4 boards, no self-pairings.
    let (status, pairings_r1) = app
        .get(&format!("/tournament/{tournament_id}/pairings/1"))
        .await;
    assert_eq!(status, StatusCode::OK);
    let boards_r1 = pairings_r1["pairings"].as_array().unwrap().clone();
    assert_eq!(boards_r1.len(), 4, "round 1: {pairings_r1}");
    for board in &boards_r1 {
        assert_ne!(board["white"], board["black"], "self-pairing: {board}");
    }

    // Use the recorded pairing board/color, not a separately derived my-match view.
    let gm_pairing = boards_r1
        .iter()
        .find(|b| b["white"] == json!("gm_wallet") || b["black"] == json!("gm_wallet"))
        .expect("GrandMaster should have a round 1 pairing");
    let gm_board = gm_pairing["board"].as_u64().unwrap();
    let gm_is_white = gm_pairing["white"] == json!("gm_wallet");

    // Give GrandMaster the sole round-1 win to avoid standings tiebreak ambiguity.
    for board in &boards_r1 {
        let board_num = board["board"].as_u64().unwrap();
        let result = if board_num == gm_board {
            if gm_is_white {
                "1-0"
            } else {
                "0-1"
            }
        } else {
            "0.5-0.5"
        };
        let (status, body) = app
            .post_json_admin(
                &format!("/admin/tournament/{tournament_id}/result"),
                &json!({ "round": 1, "board": board_num, "result": result }),
            )
            .await;
        assert_eq!(
            status,
            StatusCode::OK,
            "record result board {board_num}: {body}"
        );
    }

    // All 4 results recorded -> the service auto-starts round 2.
    let (status, round) = app
        .get(&format!("/tournament/{tournament_id}/current-round"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(round["round"], json!(2), "should auto-advance to round 2");

    let (status, pairings_r2) = app
        .get(&format!("/tournament/{tournament_id}/pairings/2"))
        .await;
    assert_eq!(status, StatusCode::OK);
    let boards_r2 = pairings_r2["pairings"].as_array().unwrap();
    assert!(!boards_r2.is_empty(), "round 2 should have pairings");
    assert!(
        boards_r2.len() <= boards_r1.len(),
        "Swiss rounds should not create more pairings than the previous round"
    );
    for board in boards_r2 {
        assert_ne!(board["white"], board["black"], "self-pairing: {board}");
    }

    // Standings reflect round 1 only (round 2 has no results yet): sorted
    // descending by score, GrandMaster alone at the top.
    let (status, standings) = app
        .get(&format!("/tournament/{tournament_id}/standings"))
        .await;
    assert_eq!(status, StatusCode::OK);
    let standings = standings.as_array().unwrap();
    assert_eq!(standings.len(), 8);
    for pair in standings.windows(2) {
        let a = pair[0]["score"].as_f64().unwrap();
        let b = pair[1]["score"].as_f64().unwrap();
        assert!(a >= b, "standings should be sorted by score: {a} then {b}");
    }
    assert_eq!(standings[0]["player_id"], json!("gm_wallet"));
    assert_eq!(standings[0]["score"], json!(1.0));
}

#[tokio::test]
async fn approve_prize_release_route_sets_the_flag() {
    let app = spawn_app().await;
    let tournament_id = 9100_u64;

    let record = TournamentRecord::new(tournament_id, "Big Prize Tournament", 0);
    app.state.tournament_store.create(record).await;

    let fetched = app
        .state
        .tournament_store
        .get(tournament_id)
        .await
        .expect("seeded tournament");
    assert!(!fetched.prize_release_approved, "should start unapproved");

    // No X-API-Key → rejected before the handler runs, same as every other
    // admin route.
    let (status, _) = app
        .post_json(
            &format!("/admin/tournament/{tournament_id}/approve-prize-release"),
            &json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(
        !app.state
            .tournament_store
            .get(tournament_id)
            .await
            .unwrap()
            .prize_release_approved,
        "unauthorized call must not have flipped the flag"
    );

    // With the admin key → approved.
    let (status, body) = app
        .post_json_admin(
            &format!("/admin/tournament/{tournament_id}/approve-prize-release"),
            &json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        app.state
            .tournament_store
            .get(tournament_id)
            .await
            .unwrap()
            .prize_release_approved,
        "approved flag should now be set"
    );

    // Unknown tournament → 404, not a silent success.
    let (status, _) = app
        .post_json_admin("/admin/tournament/999999/approve-prize-release", &json!({}))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// Mirrors what the settlement worker does when a tournament game settles on-chain:
/// it hands the orchestrator the Game's id, players and result.
async fn settle(
    tx: &tokio::sync::mpsc::Sender<OrchestratorEvent>,
    tournament_id: u64,
    game_id: u64,
    white: &str,
    black: &str,
    outcome: GameOutcome,
) {
    tx.send(OrchestratorEvent::GameEnded {
        tournament_id,
        game_id,
        white: white.to_string(),
        black: black.to_string(),
        outcome,
    })
    .await
    .unwrap();
}

/// Waits for the orchestrator (a separate task) to apply queued events.
async fn wait_until<F: Fn(&TournamentRecord) -> bool>(
    app: &TestApp,
    id: u64,
    done: F,
) -> TournamentRecord {
    for _ in 0..200 {
        let t = app.state.tournament_store.get(id).await.unwrap();
        if done(&t) {
            return t;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("orchestrator did not reach the expected state");
}

#[tokio::test]
async fn swiss_tournament_runs_end_to_end_from_settled_games() {
    use backend::signing::storage::tournament::TournamentStatus;
    use swiss_pairing::MatchResult;

    let app = spawn_app().await;
    let tournament_id = 9200_u64;
    let rounds = 3u8;

    let mut record = TournamentRecord::with_config(
        tournament_id,
        "Settled Swiss".to_string(),
        0,
        0,
        7,
        [5000, 3000, 2000, 0, 0, 0, 0, 0, 0, 0],
        TournamentFormat::Swiss { rounds },
        None,
        None,
        Some(2),
        None,
        false,
    );
    record.max_players = 7;
    app.state.tournament_store.create(record).await;
    // Odd field: every round has exactly one bye.
    let players = ["a", "b", "c", "d", "e", "f", "g"];
    seed_players(
        &app,
        tournament_id,
        players
            .iter()
            .enumerate()
            .map(|(i, p)| (*p, 2000 - i as u32 * 50)),
    )
    .await;

    let (status, body) = app
        .post_json_admin(
            &format!("/admin/tournament/{tournament_id}/initialize-swiss"),
            &json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "initialize-swiss: {body}");

    // No chain config: this exercises the off-chain half of the orchestrator.
    let tx = spawn_orchestrator(
        (*app.state.tournament_store).clone(),
        app.state.swiss_service.clone(),
        None,
    );

    for round in 1..=rounds {
        let (_, pairings) = app
            .get(&format!("/tournament/{tournament_id}/pairings/{round}"))
            .await;
        let boards = pairings["pairings"].as_array().unwrap().clone();
        assert_eq!(boards.len(), 3, "round {round}: {pairings}");
        assert_eq!(
            pairings["byes"].as_array().unwrap().len(),
            1,
            "round {round}"
        );

        // Every paired player is told to play their board's deterministic game.
        for b in &boards {
            let board = b["board"].as_u64().unwrap() as u16;
            let game_id = TournamentStore::swiss_game_id(tournament_id, round, board).unwrap();
            for (side, color) in [("white", "white"), ("black", "black")] {
                let player = b[side].as_str().unwrap();
                let (_, s) = app
                    .get(&format!(
                        "/tournament/{tournament_id}/my-status?player={player}"
                    ))
                    .await;
                assert_eq!(
                    s["state"],
                    json!("MatchReady"),
                    "{player} round {round}: {s}"
                );
                assert_eq!(s["match"]["game_id"], json!(game_id), "{player}: {s}");
                assert_eq!(s["match"]["your_color"], json!(color));
            }
        }

        // The round cannot be skipped while boards are open.
        let (status, _) = app
            .post_json_admin(
                &format!("/admin/tournament/{tournament_id}/advance-round"),
                &json!({}),
            )
            .await;
        assert_eq!(
            status,
            StatusCode::CONFLICT,
            "round {round} skipped while open"
        );

        // A game whose on-chain players don't match the pairing is ignored.
        let first = &boards[0];
        let first_board = first["board"].as_u64().unwrap() as u16;
        let first_game = TournamentStore::swiss_game_id(tournament_id, round, first_board).unwrap();
        settle(
            &tx,
            tournament_id,
            first_game,
            first["black"].as_str().unwrap(),
            first["white"].as_str().unwrap(),
            GameOutcome::WhiteWin,
        )
        .await;

        for (i, b) in boards.iter().enumerate() {
            let board = b["board"].as_u64().unwrap() as u16;
            let result = if i == 0 {
                GameOutcome::WhiteWin
            } else {
                GameOutcome::Draw
            };
            let game_id = TournamentStore::swiss_game_id(tournament_id, round, board).unwrap();
            let (white, black) = (b["white"].as_str().unwrap(), b["black"].as_str().unwrap());
            settle(&tx, tournament_id, game_id, white, black, result).await;
            // A settlement retry for the same game is harmless.
            settle(&tx, tournament_id, game_id, white, black, result).await;

            if round == rounds && i + 1 < boards.len() {
                let t = wait_until(&app, tournament_id, |t| {
                    t.swiss().unwrap().results.len() == (round as usize - 1) * 3 + i + 1
                })
                .await;
                assert_eq!(
                    t.status,
                    TournamentStatus::Active,
                    "final round must not complete before its last board"
                );
            }
        }

        let t = wait_until(&app, tournament_id, |t| {
            let sd = t.swiss().unwrap();
            sd.round_complete(round) && (sd.current_round > round || round == rounds)
        })
        .await;
        let sd = t.swiss().unwrap();
        assert_eq!(
            sd.results.len(),
            round as usize * 3,
            "one result per board, no duplicates"
        );
        let first_result = sd
            .results
            .iter()
            .find(|(r, b, _)| *r == round && *b == first_board)
            .unwrap();
        assert_eq!(
            first_result.2,
            MatchResult::WhiteWin,
            "mismatched-player result was ignored"
        );
    }

    let t = app.state.tournament_store.get(tournament_id).await.unwrap();
    assert_eq!(t.status, TournamentStatus::Completed);
    let standings = &t.swiss().unwrap().standings;
    assert_eq!(t.winner.as_deref(), Some(standings[0].player_id.as_str()));
    assert_eq!(
        t.seventh_place.as_deref(),
        Some(standings[6].player_id.as_str())
    );
    let mut byes: Vec<String> = t
        .swiss()
        .unwrap()
        .rounds
        .iter()
        .flat_map(|r| r.byes.clone())
        .collect();
    byes.sort();
    byes.dedup();
    assert_eq!(byes.len(), rounds as usize, "nobody gets a second bye");

    // A completed event takes no further results.
    let (status, _) = app
        .post_json_admin(
            &format!("/admin/tournament/{tournament_id}/result"),
            &json!({ "round": rounds, "board": 1, "result": "0-1" }),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn admin_views_show_swiss_boards_and_hide_private_fields() {
    let app = spawn_app().await;
    let tournament_id = 9300_u64;
    let mut record = TournamentRecord::with_config(
        tournament_id,
        "Private Swiss".to_string(),
        0,
        0,
        4,
        [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0],
        TournamentFormat::Swiss { rounds: 2 },
        None,
        None,
        Some(2),
        None,
        false,
    );
    record.password_hash = Some("argon2-hash-must-not-leak".into());
    record.node_ids.insert("p0".into(), "node-p0".into());
    app.state.tournament_store.create(record).await;
    seed_players(
        &app,
        tournament_id,
        [("p0", 2000), ("p1", 1900), ("p2", 1800), ("p3", 1700)],
    )
    .await;

    let (status, _) = app
        .post_json_admin(
            &format!("/admin/tournament/{tournament_id}/initialize-swiss"),
            &json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, detail) = app.get(&format!("/api/tournament/{tournament_id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(detail.get("password_hash").is_none(), "{detail}");
    assert!(detail.get("node_ids").is_none(), "{detail}");
    assert_eq!(detail["is_private"], json!(true));

    let (status, bracket) = app
        .get(&format!("/api/tournament/{tournament_id}/bracket"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        bracket["current_round"],
        json!(0),
        "0-based like bracket rounds"
    );
    let matches = bracket["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 2, "{bracket}");
    for m in matches {
        assert_eq!(m["round"], json!(0));
        assert_eq!(m["status"], json!("Active"));
        let board = m["board"].as_u64().unwrap() as u16;
        assert_eq!(
            m["game_id"],
            json!(TournamentStore::swiss_game_id(tournament_id, 1, board))
        );
    }

    // A decided board shows as completed with its winner.
    let board = matches[0]["board"].as_u64().unwrap();
    let white = matches[0]["player_white"].clone();
    let (status, _) = app
        .post_json_admin(
            &format!("/admin/tournament/{tournament_id}/result"),
            &json!({ "round": 1, "board": board, "result": "1-0" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (_, bracket) = app
        .get(&format!("/api/tournament/{tournament_id}/bracket"))
        .await;
    let decided = bracket["matches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["board"].as_u64() == Some(board))
        .unwrap();
    assert_eq!(decided["status"], json!("Completed"));
    assert_eq!(decided["winner"], white);
}

#[tokio::test]
async fn start_route_explains_partial_brackets_and_starts_swiss() {
    let app = spawn_app().await;

    // A 4-player bracket with 3 registered cannot start on-chain: clear 409.
    let bracket_id = 9400_u64;
    app.state
        .tournament_store
        .create(TournamentRecord::with_config(
            bracket_id,
            "Bracket".to_string(),
            0,
            0,
            4,
            [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0],
            TournamentFormat::SingleElimination,
            None,
            None,
            None,
            None,
            false,
        ))
        .await;
    seed_players(&app, bracket_id, [("a", 2000), ("b", 1900), ("c", 1800)]).await;
    let (status, _) = app
        .post_json_admin(&format!("/admin/tournament/{bracket_id}/start"), &json!({}))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // Swiss starts at its minimum through the same route.
    let swiss_id = 9401_u64;
    app.state
        .tournament_store
        .create(TournamentRecord::with_config(
            swiss_id,
            "Swiss".to_string(),
            0,
            0,
            8,
            [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0],
            TournamentFormat::Swiss { rounds: 3 },
            None,
            None,
            Some(2),
            None,
            false,
        ))
        .await;
    seed_players(
        &app,
        swiss_id,
        [("p0", 2000), ("p1", 1900), ("p2", 1800), ("p3", 1700)],
    )
    .await;
    let (status, body) = app
        .post_json_admin(&format!("/admin/tournament/{swiss_id}/start"), &json!({}))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, round) = app
        .get(&format!("/tournament/{swiss_id}/current-round"))
        .await;
    assert_eq!(round["round"], json!(1));

    // A started tournament cannot be started again.
    let (status, _) = app
        .post_json_admin(&format!("/admin/tournament/{swiss_id}/start"), &json!({}))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

/// Match `index` as the public bracket view shows it.
async fn bracket_match(app: &TestApp, id: u64, index: usize) -> Value {
    let (status, bracket) = app.get(&format!("/api/tournament/{id}/bracket")).await;
    assert_eq!(status, StatusCode::OK);
    bracket["matches"][index].clone()
}

async fn my_status(app: &TestApp, id: u64, player: &str) -> Value {
    let (status, s) = app
        .get(&format!("/tournament/{id}/my-status?player={player}"))
        .await;
    assert_eq!(status, StatusCode::OK);
    s
}

#[tokio::test]
async fn bracket_runs_end_to_end_from_settled_games_replaying_draws() {
    use backend::signing::elimination::MAX_REPLAYS;
    use backend::signing::storage::tournament::{ResultSource, TournamentStatus};

    let app = spawn_app().await;
    let tournament_id = 9500_u64;
    app.state
        .tournament_store
        .create(TournamentRecord::with_config(
            tournament_id,
            "Settled Bracket".to_string(),
            0,
            0,
            4,
            [5000, 3000, 1500, 500, 0, 0, 0, 0, 0, 0],
            TournamentFormat::SingleElimination,
            None,
            None,
            None,
            None,
            false,
        ))
        .await;
    // Registered out of rating order; starting seeds them.
    seed_players(
        &app,
        tournament_id,
        [("d", 1700), ("b", 1900), ("a", 2000), ("c", 1800)],
    )
    .await;
    // What the scheduler does once the on-chain start has landed.
    app.state
        .tournament_store
        .start_tournament(tournament_id)
        .await
        .unwrap();
    // No chain config: this exercises the off-chain half of the orchestrator.
    let tx = spawn_orchestrator(
        (*app.state.tournament_store).clone(),
        app.state.swiss_service.clone(),
        None,
    );

    // Round one plays the best seed against the worst.
    let semi = bracket_match(&app, tournament_id, 0).await;
    assert_eq!(semi["player_white"], json!("a"), "{semi}");
    assert_eq!(semi["player_black"], json!("d"), "{semi}");
    let first_game = semi["game_id"].as_u64().unwrap();
    let s = my_status(&app, tournament_id, "a").await;
    assert_eq!(s["state"], json!("MatchReady"), "{s}");
    assert_eq!(s["match"]["game_id"], json!(first_game));
    assert_eq!(s["match"]["your_color"], json!("white"));

    // A drawn game is replayed under a new id with colours reversed.
    settle(&tx, tournament_id, first_game, "a", "d", GameOutcome::Draw).await;
    wait_until(&app, tournament_id, |t| {
        t.elimination().unwrap().matches[0].drawn_games == 1
    })
    .await;
    let s = my_status(&app, tournament_id, "a").await;
    assert_eq!(s["state"], json!("MatchReady"), "{s}");
    assert_eq!(s["match"]["your_color"], json!("black"));
    let replay = s["match"]["game_id"].as_u64().unwrap();
    assert_ne!(replay, first_game);

    // A retried settlement of the drawn game changes nothing; the replay decides.
    settle(&tx, tournament_id, first_game, "a", "d", GameOutcome::Draw).await;
    settle(&tx, tournament_id, replay, "d", "a", GameOutcome::WhiteWin).await;
    let t = wait_until(&app, tournament_id, |t| {
        t.elimination().unwrap().matches[0].is_decided()
    })
    .await;
    let decided = &t.elimination().unwrap().matches[0];
    assert_eq!(decided.winner.as_deref(), Some("d"));
    assert_eq!(
        decided.drawn_games, 1,
        "the retried draw is not counted twice"
    );
    assert_eq!(t.fourth_place.as_deref(), Some("a"));
    assert_eq!(
        my_status(&app, tournament_id, "a").await["state"],
        json!("Eliminated")
    );

    // Every game of the other semifinal drawn: the higher seed goes through.
    for game in 0..=MAX_REPLAYS {
        let m = bracket_match(&app, tournament_id, 1).await;
        let (white, black) = (
            m["player_white"].as_str().unwrap().to_string(),
            m["player_black"].as_str().unwrap().to_string(),
        );
        let expected_white = if game % 2 == 0 { "b" } else { "c" };
        assert_eq!(white, expected_white, "colours alternate: {m}");
        let game_id = m["game_id"].as_u64().unwrap();
        settle(
            &tx,
            tournament_id,
            game_id,
            &white,
            &black,
            GameOutcome::Draw,
        )
        .await;
        wait_until(&app, tournament_id, |t| {
            t.elimination().unwrap().matches[1].drawn_games == game + 1
        })
        .await;
    }
    let t = app.state.tournament_store.get(tournament_id).await.unwrap();
    let tiebreak = &t.elimination().unwrap().matches[1];
    assert_eq!(tiebreak.winner.as_deref(), Some("b"));
    assert_eq!(tiebreak.result_source, Some(ResultSource::SeedTiebreak));
    assert_eq!(t.third_place.as_deref(), Some("c"));

    // The final: a game with the wrong players is ignored, the right one decides.
    let final_match = bracket_match(&app, tournament_id, 2).await;
    let (white, black) = (
        final_match["player_white"].as_str().unwrap().to_string(),
        final_match["player_black"].as_str().unwrap().to_string(),
    );
    assert_eq!(
        (white.as_str(), black.as_str()),
        ("d", "b"),
        "{final_match}"
    );
    let final_game = final_match["game_id"].as_u64().unwrap();
    assert_eq!(
        my_status(&app, tournament_id, "b").await["state"],
        json!("MatchReady")
    );
    settle(
        &tx,
        tournament_id,
        final_game,
        &black,
        &white,
        GameOutcome::WhiteWin,
    )
    .await;
    settle(
        &tx,
        tournament_id,
        final_game,
        &white,
        &black,
        GameOutcome::BlackWin,
    )
    .await;
    let t = wait_until(&app, tournament_id, |t| {
        t.status == TournamentStatus::Completed
    })
    .await;
    assert_eq!(t.winner.as_deref(), Some("b"));
    assert_eq!(t.second_place.as_deref(), Some("d"));
    assert_eq!(
        my_status(&app, tournament_id, "b").await["state"],
        json!("Champion")
    );

    // Spectators see each match's deciding game, all finished.
    let (status, games) = app
        .get(&format!("/api/tournament/{tournament_id}/games"))
        .await;
    assert_eq!(status, StatusCode::OK);
    let games = games["games"].as_array().unwrap();
    assert_eq!(games.len(), 3, "{games:?}");
    assert!(
        games.iter().all(|g| g["state"] == json!("finished")),
        "{games:?}"
    );

    // A completed bracket takes no further results.
    let (status, _) = app
        .post_json_admin(
            &format!("/admin/tournament/{tournament_id}/record-result"),
            &json!({ "match_index": 2, "winner": "d", "loser": "b" }),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn admin_results_and_round_advances_follow_the_format() {
    let app = spawn_app().await;
    let (bracket_id, swiss_id) = (9600_u64, 9601_u64);
    for (id, format) in [
        (bracket_id, TournamentFormat::SingleElimination),
        (swiss_id, TournamentFormat::Swiss { rounds: 2 }),
    ] {
        app.state
            .tournament_store
            .create(TournamentRecord::with_config(
                id,
                "Admin".to_string(),
                0,
                0,
                4,
                [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0],
                format,
                None,
                None,
                Some(2),
                None,
                false,
            ))
            .await;
        seed_players(
            &app,
            id,
            [("p0", 2000), ("p1", 1900), ("p2", 1800), ("p3", 1700)],
        )
        .await;
    }
    app.state
        .tournament_store
        .start_tournament(bracket_id)
        .await
        .unwrap();
    let (status, _) = app
        .post_json_admin(
            &format!("/admin/tournament/{swiss_id}/initialize-swiss"),
            &json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let result =
        |winner: &str, loser: &str| json!({ "match_index": 0, "winner": winner, "loser": loser });
    let record_result = |id: u64| format!("/admin/tournament/{id}/record-result");

    // Bracket results are checked against the match's seats.
    let (status, _) = app
        .post_json_admin(&record_result(bracket_id), &result("p0", "p1"))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "p1 is not in match 0");
    let (status, _) = app
        .post_json_admin(&record_result(bracket_id), &result("p3", "p0"))
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = app
        .post_json_admin(&record_result(bracket_id), &result("p0", "p3"))
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "match 0 is already decided");
    let t = app.state.tournament_store.get(bracket_id).await.unwrap();
    assert_eq!(
        t.elimination().unwrap().matches[2].player_white.as_deref(),
        Some("p3")
    );

    // Swiss boards have their own result route; brackets have no rounds to advance.
    let (status, _) = app
        .post_json_admin(&record_result(swiss_id), &result("p0", "p3"))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = app
        .post_json_admin(
            &format!("/admin/tournament/{bracket_id}/advance-round"),
            &json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

/// Registers players directly in the store. `POST /tournament/{id}/join` requires a
/// wallet JWT matching the player (`RequireWallet`), which these chain-free tests
/// don't mint; everything after registration still goes over HTTP.
async fn seed_players(app: &TestApp, id: u64, players: impl IntoIterator<Item = (&str, u32)>) {
    let players: Vec<(String, u32)> = players
        .into_iter()
        .map(|(p, elo)| (p.to_string(), elo))
        .collect();
    assert!(
        app.state
            .tournament_store
            .update(id, |t| {
                for (p, elo) in players {
                    t.players.push(p);
                    t.player_elos.push(elo);
                }
            })
            .await
    );
}
