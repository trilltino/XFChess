//! Tournament API routes for 2-256 player single-elimination and Swiss tournaments.
//!
//! This module provides HTTP endpoints for tournament management:
//! - Admin endpoints: create, record results, set match game IDs
//! - Player endpoints: list tournaments, join, get my match, get bracket, register node ID
//! - Gossip endpoints: subscribe to tournament updates, get bootstrap peers
//!
//! Tournaments use ELO-based seeding and support power-of-2 player counts.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use serde::{Deserialize, Serialize};
use solana_sdk::{message::Message, pubkey::Pubkey, signature::Signer, transaction::Transaction};
use std::collections::HashMap;
use std::str::FromStr;
use tracing::{error, info, warn};

use crate::db::repository::GameRepository;
use crate::signing::solana::{
    initialize_escrow_ix, initialize_shards_ix, initialize_tournament_ix, record_result_ix,
    sign_and_submit,
};
use crate::signing::storage::tournament::{
    MatchStatus, TournamentFormat, TournamentRecord, TournamentStatus,
};
use crate::signing::{AppState, TournamentTrigger};

// ── Request / Response types ──────────────────────────────────────────────────

/// Request to create a new tournament.
#[derive(Deserialize, Serialize)]
pub struct CreateTournamentReq {
    pub tournament_id: u64,
    pub name: String,
    /// Total entry fee in lamports. If omitted, auto-calculated from live SOL/GBP rate (£3.00 = 50p platform + £2.50 prize).
    pub entry_fee_lamports: Option<u64>,
    /// Platform fee portion in lamports. If omitted, auto-calculated as 50p from live rate.
    pub platform_fee_lamports: Option<u64>,
    /// Max players: 2, 4, 8, 16, 32, 64, 128, or 256
    pub max_players: u16,
    /// Tournament format: "SingleElimination" or "Swiss"
    #[serde(default = "default_format")]
    pub format: String,
    /// Number of Swiss rounds (required for Swiss format)
    pub swiss_rounds: Option<u8>,
    /// Minimum ELO rating for players (optional)
    pub elo_min: Option<u32>,
    /// Maximum ELO rating for players (optional)
    pub elo_max: Option<u32>,
    /// Minimum players required to start tournament (optional)
    pub min_players: Option<u16>,
    /// Prize distribution in basis points [1st-10th]. Default: competitive split based on max_players
    pub prize_shares: Option<[u16; 10]>,
    /// Winner takes all mode (overrides prize_shares with [10000, 0, 0, 0, 0, 0, 0, 0, 0, 0])
    #[serde(default)]
    pub winner_takes_all: bool,
    /// Unix timestamp for when the tournament is scheduled to open (None = open immediately)
    pub scheduled_at: Option<i64>,
    /// Whether CACF KYC verification is required to join
    #[serde(default)]
    pub kyc_required: bool,
}

fn default_format() -> String {
    "SingleElimination".to_string()
}

/// Request to register a player's P2P node ID.
#[derive(Deserialize, Serialize)]
pub struct RegisterNodeReq {
    pub player: String,
    pub node_id: String,
}

/// Request to subscribe to tournament gossip updates.
#[derive(Deserialize, Serialize)]
pub struct SubscribeNodeReq {
    pub player: String,
    pub node_id: String,
}

/// Response for tournament subscription.
#[derive(Serialize)]
pub struct SubscribeNodeRes {
    pub ok: bool,
    /// Bootstrap peer node IDs to connect to
    pub bootstrap_peers: Vec<String>,
    /// Tournament topic URL
    pub topic_url: String,
}

/// Request to record a match result.
#[derive(Deserialize, Serialize)]
pub struct RecordResultReq {
    pub match_index: usize,
    pub winner: String,
    pub loser: String,
    /// Optional: "forfeit" | "no_show" — stored for audit; normal win/loss rules apply
    #[serde(default)]
    pub reason: Option<String>,
}

/// Request to reseed players in a tournament (pre-start only).
#[derive(Deserialize, Serialize)]
pub struct ReseedReq {
    /// Ordered player list (wallet pubkeys) after reseeding.
    pub players: Vec<String>,
}

/// Request to set the game ID for a match.
#[derive(Deserialize, Serialize)]
pub struct SetMatchGameIdReq {
    pub match_index: usize,
    pub game_id: u64,
}

/// Tournament summary for listing.
#[derive(Serialize)]
pub struct TournamentSummary {
    pub tournament_id: u64,
    pub name: String,
    pub entry_fee_lamports: u64,
    pub prize_pool: u64,
    pub max_players: u16,
    pub registered: usize,
    pub status: String,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// POST /tournament/{id}/subscribe-node - Subscribe to tournament gossip updates
///
/// Called by game client to register for gossip updates and receive bootstrap peers.
async fn subscribe_node(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<SubscribeNodeReq>,
) -> Result<Json<SubscribeNodeRes>, StatusCode> {
    // Register player's node_id for the tournament
    let ok = state
        .tournament_store
        .register_node_id(id, req.player.clone(), req.node_id.clone())
        .await;
    if !ok {
        return Err(StatusCode::NOT_FOUND);
    }

    // Get bootstrap peers for the player
    let bootstrap_peers = state
        .tournament_gossip
        .get_bootstrap_peers(id, &req.player)
        .await;

    // Format peer IDs as hex strings
    let peer_strings: Vec<String> = bootstrap_peers
        .iter()
        .map(|p| hex::encode(p.as_bytes()))
        .collect();

    // Increment subscriber count
    state.tournament_gossip.increment_subscribers(id).await;

    info!(
        "[tournament] {} subscribed to gossip for tournament {} ({} bootstrap peers)",
        req.player,
        id,
        peer_strings.len()
    );

    Ok(Json(SubscribeNodeRes {
        ok: true,
        bootstrap_peers: peer_strings,
        topic_url: format!("/swiss/{}", id),
    }))
}

/// GET /tournament/{id}/bootstrap-peers?player={pubkey} - Get bootstrap peers for a player
///
/// Returns list of peer node IDs to connect to for P2P gossip.
async fn get_bootstrap_peers(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let player = params.get("player").ok_or(StatusCode::BAD_REQUEST)?;

    // Verify tournament exists
    if state.tournament_store.get(id).await.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Get bootstrap peers
    let bootstrap_peers = state
        .tournament_gossip
        .get_bootstrap_peers(id, player)
        .await;

    // Format peer IDs as hex strings
    let peer_strings: Vec<String> = bootstrap_peers
        .iter()
        .map(|p| hex::encode(p.as_bytes()))
        .collect();

    Ok(Json(serde_json::json!({
        "tournament_id": id,
        "player": player,
        "bootstrap_peers": peer_strings,
        "count": peer_strings.len(),
    })))
}

/// POST /admin/tournament/create - Creates a new tournament.
async fn create_tournament(
    State(state): State<AppState>,
    Json(req): Json<CreateTournamentReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = &state.tournament_store;
    // Parse tournament format
    let format = match req.format.as_str() {
        "Swiss" => {
            let rounds = req.swiss_rounds.ok_or_else(|| StatusCode::BAD_REQUEST)?;
            TournamentFormat::Swiss { rounds }
        }
        "SingleElimination" | "" => TournamentFormat::SingleElimination,
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    // Validate the player count. Single-elimination needs a full power-of-2
    // bracket (the on-chain program enforces the same list); Swiss just needs
    // at least one pairing.
    const VALID_PLAYER_COUNTS: [u16; 8] = [2, 4, 8, 16, 32, 64, 128, 256];
    match format {
        TournamentFormat::SingleElimination => {
            if !VALID_PLAYER_COUNTS.contains(&req.max_players) {
                return Err(StatusCode::BAD_REQUEST);
            }
        }
        TournamentFormat::Swiss { .. } => {
            if req.max_players < 2 {
                return Err(StatusCode::BAD_REQUEST);
            }
        }
    }

    // Auto-calculate fees from live SOL/GBP rate if not explicitly provided.
    // Standard: 50p platform fee + £2.50 prize contribution = £3.00 total entry.
    let platform_fee_lamports = match req.platform_fee_lamports {
        Some(v) => v,
        None => state.rate_cache.gbp_to_lamports(0.50).await.unwrap_or(0),
    };
    let entry_fee_lamports = match req.entry_fee_lamports {
        Some(v) => v,
        None => state.rate_cache.gbp_to_lamports(3.00).await.unwrap_or(0),
    };

    // Default competitive prize shares based on tournament size
    let default_shares = if req.winner_takes_all {
        [10000, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    } else {
        match req.max_players {
            0..=2 => [7000, 3000, 0, 0, 0, 0, 0, 0, 0, 0], // Head-to-head: 70/30% (no 3rd place)
            3..=64 => [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0], // Top 3: 60/30/10%
            128 => [5000, 2500, 1500, 500, 500, 0, 0, 0, 0, 0], // Top 5: 50/25/15/5/5%
            256 => [4000, 2000, 1200, 800, 600, 400, 300, 200, 200, 300], // Top 10: 40/20/12/8/6/4/3/2/2/3%
            _ => [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0],                 // Default to 64 and below
        }
    };

    let prize_shares = req.prize_shares.unwrap_or(default_shares);

    // ── On-chain setup (3 sequential VPS-signed transactions) ────────────────
    // All three must confirm before writing to the store. Failure returns 500
    // without any store mutation so the admin can safely retry.
    let program_id = Pubkey::from_str(&state.config.program_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let authority = &*state.vps_authority;
    let rpc = crate::signing::solana::make_rpc(&state.config.solana_rpc_url);

    // 1. initialize_tournament
    let ix1 = initialize_tournament_ix(
        &program_id,
        &authority.pubkey(),
        req.tournament_id,
        &req.name,
        entry_fee_lamports,
        platform_fee_lamports,
        req.max_players,
        match format {
            TournamentFormat::Swiss { .. } => 1,
            _ => 0,
        },
        match format {
            TournamentFormat::Swiss { rounds } => rounds,
            _ => 0,
        },
        req.elo_min.unwrap_or(0),
        req.elo_max.unwrap_or(u32::MAX),
        req.min_players.unwrap_or(req.max_players),
        prize_shares,
        false,
        &authority.pubkey(),
    );
    sign_and_submit(&rpc, authority, &[ix1]).map_err(|e| {
        error!(
            "[tournament] initialize_tournament tx failed for {}: {}",
            req.tournament_id, e
        );
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // 2. initialize_escrow
    let ix2 = initialize_escrow_ix(&program_id, req.tournament_id, &authority.pubkey());
    sign_and_submit(&rpc, authority, &[ix2]).map_err(|e| {
        error!(
            "[tournament] initialize_escrow tx failed for {}: {}",
            req.tournament_id, e
        );
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // 3. initialize_shards (variant chosen by max_players)
    let ix3 = initialize_shards_ix(
        &program_id,
        req.tournament_id,
        req.max_players,
        &authority.pubkey(),
    );
    sign_and_submit(&rpc, authority, &[ix3]).map_err(|e| {
        error!(
            "[tournament] initialize_shards tx failed for {}: {}",
            req.tournament_id, e
        );
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // ── Store write (only after all 3 txs confirmed) ──────────────────────────
    let record = TournamentRecord::with_config(
        req.tournament_id,
        req.name.clone(),
        entry_fee_lamports,
        platform_fee_lamports,
        req.max_players,
        prize_shares,
        format.clone(),
        req.elo_min,
        req.elo_max,
        req.min_players,
        req.scheduled_at,
        req.kyc_required,
    );
    store.create(record).await;

    info!("[tournament] Created tournament {} '{}' ({} players, format: {:?}, entry: {} lamports, on-chain PDAs initialized)",
        req.tournament_id, req.name, req.max_players, format.clone(), entry_fee_lamports);

    Ok(Json(serde_json::json!({
        "ok": true,
        "tournament_id": req.tournament_id,
        "max_players": req.max_players,
        "format": format.clone(),
        "prize_shares": prize_shares,
        "entry_fee_lamports": entry_fee_lamports,
        "platform_fee_lamports": platform_fee_lamports,
        "scheduled_at": req.scheduled_at,
        "kyc_required": req.kyc_required,
    })))
}

/// GET /tournaments - Lists all tournaments.
async fn list_tournaments(State(state): State<AppState>) -> Json<Vec<TournamentSummary>> {
    let store = &state.tournament_store;
    let all = store.list().await;
    let summaries = all
        .into_iter()
        .map(|t| TournamentSummary {
            tournament_id: t.tournament_id,
            name: t.name,
            entry_fee_lamports: t.entry_fee_lamports,
            prize_pool: t.prize_pool,
            max_players: t.max_players,
            registered: t.players.len(),
            status: format!("{:?}", t.status),
        })
        .collect();
    Json(summaries)
}

/// GET /tournaments/my?player=<pubkey> - Lists tournaments a specific player has registered for.
async fn list_my_tournaments(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<TournamentSummary>>, StatusCode> {
    let player = params.get("player").ok_or(StatusCode::BAD_REQUEST)?;
    let store = &state.tournament_store;

    let all = store.list().await;
    let my_tournaments = all
        .into_iter()
        .filter(|t| t.players.contains(player))
        .map(|t| TournamentSummary {
            tournament_id: t.tournament_id,
            name: t.name,
            entry_fee_lamports: t.entry_fee_lamports,
            prize_pool: t.prize_pool,
            max_players: t.max_players,
            registered: t.players.len(),
            status: format!("{:?}", t.status),
        })
        .collect();

    Ok(Json(my_tournaments))
}

/// GET /tournament/:id - Gets tournament details.
async fn get_tournament(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<TournamentRecord>, StatusCode> {
    state
        .tournament_store
        .get(id)
        .await
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// POST /tournament/:id/register-node - Registers a player's P2P node ID.
async fn register_node(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<RegisterNodeReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let ok = state
        .tournament_store
        .register_node_id(id, req.player.clone(), req.node_id.clone())
        .await;
    if !ok {
        return Err(StatusCode::NOT_FOUND);
    }
    info!(
        "[tournament] {} registered node_id for tournament {}",
        req.player, id
    );
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// GET /tournament/:id/my-match?player=<pubkey> - Gets a player's current match.
async fn get_my_match(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = &state.tournament_store;
    let player = params.get("player").ok_or(StatusCode::BAD_REQUEST)?;
    let tournament = store.get(id).await.ok_or(StatusCode::NOT_FOUND)?;
    match tournament.match_for_player(player) {
        Some(a) => Ok(Json(serde_json::json!({
            "found": true,
            "match_index": a.match_index,
            "round": a.round,
            "board": a.board,
            "game_id": a.game_id,
            "opponent_pubkey": a.opponent_pubkey,
            "opponent_node_id": a.opponent_node_id,
            "your_color": a.your_color,
            "status": format!("{:?}", a.status),
            "is_bye": a.is_bye,
        }))),
        None => Ok(Json(serde_json::json!({ "found": false }))),
    }
}

/// GET /tournament/:id/bracket - Gets the tournament bracket.
async fn get_bracket(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = state
        .tournament_store
        .get(id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;
    // Current round = lowest round that still has an uncompleted match.
    // 255 marks a finished bracket (final completed).
    let final_completed = t
        .matches
        .last()
        .and_then(|m| m.as_ref())
        .map_or(false, |m| m.status == MatchStatus::Completed);
    let current_round = if final_completed {
        255u8
    } else {
        t.matches
            .iter()
            .flatten()
            .filter(|m| m.status != MatchStatus::Completed)
            .map(|m| m.round)
            .min()
            .unwrap_or(0)
    };

    let round_deadline_at = t.swiss_data.as_ref().and_then(|s| s.round_deadline_at);

    Ok(Json(serde_json::json!({
        "tournament_id": t.tournament_id,
        "status": format!("{:?}", t.status),
        "max_players": t.max_players,
        "players": t.players,
        "matches": t.matches,
        "winner": t.winner,
        "second_place": t.second_place,
        "third_place": t.third_place,
        "fourth_place": t.fourth_place,
        "fifth_place": t.fifth_place,
        "sixth_place": t.sixth_place,
        "seventh_place": t.seventh_place,
        "eighth_place": t.eighth_place,
        "ninth_place": t.ninth_place,
        "tenth_place": t.tenth_place,
        "prize_shares": t.prize_shares,
        "current_round": current_round,
        "round_deadline_at": round_deadline_at,
    })))
}

/// POST /admin/tournament/:id/record-result - Records a match result.
async fn record_result(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<RecordResultReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = &state.tournament_store;
    let tournament = store.get(id).await.ok_or(StatusCode::NOT_FOUND)?;
    if req.match_index >= tournament.matches.len() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let ok = store
        .record_result(id, req.match_index, req.winner.clone(), req.loser.clone())
        .await;
    if !ok {
        return Err(StatusCode::NOT_FOUND);
    }
    if let Some(ref reason) = req.reason {
        info!(
            "[tournament] Match {} of tournament {} won by {} (reason: {})",
            req.match_index, id, req.winner, reason
        );
    } else {
        info!(
            "[tournament] Match {} of tournament {} won by {}",
            req.match_index, id, req.winner
        );
    }

    // ── Mirror result on-chain (best-effort — store is already updated) ───────
    if let (Ok(program_id), Ok(winner_pk), Ok(loser_pk)) = (
        Pubkey::from_str(&state.config.program_id),
        Pubkey::from_str(&req.winner),
        Pubkey::from_str(&req.loser),
    ) {
        let authority = &*state.vps_authority;
        let rpc = crate::signing::solana::make_rpc(&state.config.solana_rpc_url);

        let ix = record_result_ix(
            &program_id,
            id,
            req.match_index as u16,
            &winner_pk,
            &loser_pk,
            &authority.pubkey(),
        );
        if let Err(e) = sign_and_submit(&rpc, authority, &[ix]) {
            error!(
                "[tournament] record_result on-chain failed for match {} of tournament {}: {}",
                req.match_index, id, e
            );
        } else if let Some((next_idx, _slot)) = store.get(id).await.and_then(|t| {
            t.matches
                .get(req.match_index)
                .and_then(|m| m.as_ref())
                .and_then(|m| m.next_match_for_winner.map(|n| (n, m.next_match_slot)))
        }) {
            // Mirror the store-side advancement: push the winner into their
            // next-round match on-chain as well (best-effort).
            let ix = crate::signing::solana::advance_winner_ix(
                &program_id,
                id,
                req.match_index as u16,
                next_idx,
                &authority.pubkey(),
            );
            if let Err(e) = sign_and_submit(&rpc, authority, &[ix]) {
                error!(
                    "[tournament] advance_winner on-chain failed for match {} -> {} of tournament {}: {}",
                    req.match_index, next_idx, id, e
                );
            }
        }
    }

    Ok(Json(serde_json::json!({ "ok": true })))
}

/// POST /admin/tournament/:id/advance-round — manually advance stuck Swiss round.
/// Should only be called when all matches in the current round are Completed but
/// the scheduler hasn't auto-advanced (e.g., backend restart cleared in-memory state).
async fn advance_round(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = &state.tournament_store;
    let tournament = store.get(id).await.ok_or(StatusCode::NOT_FOUND)?;

    // Verify all current-round matches are completed before forcing advance
    let current_round = tournament
        .swiss_data
        .as_ref()
        .map(|s| s.current_round)
        .unwrap_or(0);
    let all_done = tournament
        .matches
        .iter()
        .flatten()
        .filter(|m| m.round == current_round as u8)
        .all(|m| m.status == crate::signing::storage::tournament::MatchStatus::Completed);

    if !all_done {
        warn!("[tournament] advance-round blocked — not all round-{} matches are complete for tournament {}", current_round, id);
        return Err(StatusCode::CONFLICT);
    }

    // Ask the SwissService to pair the next round
    match state.swiss_service.start_round(id).await {
        Ok(round) => {
            info!(
                "[tournament] Admin advanced tournament {} to round {}",
                id, round.round
            );
            Ok(Json(
                serde_json::json!({ "ok": true, "new_round": round.round }),
            ))
        }
        Err(e) => {
            warn!("[tournament] advance-round failed for {}: {:?}", id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// POST /admin/tournament/:id/reseed — reorder player list before tournament starts.
async fn reseed_players(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<ReseedReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = &state.tournament_store;
    let updated = store
        .update(id, |t| {
            if t.status != crate::signing::storage::tournament::TournamentStatus::Registration {
                return; // only allowed pre-start
            }
            t.players = req.players.clone();
        })
        .await;
    if !updated {
        return Err(StatusCode::NOT_FOUND);
    }
    info!("[tournament] Players reseeded for tournament {}", id);
    Ok(Json(
        serde_json::json!({ "ok": true, "player_count": req.players.len() }),
    ))
}

/// POST /admin/tournament/:id/set-match-game-id - Sets the game ID for a match.
async fn set_match_game_id(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<SetMatchGameIdReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = &state.tournament_store;
    let tournament = store.get(id).await.ok_or(StatusCode::NOT_FOUND)?;
    if req.match_index >= tournament.matches.len() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let ok = store
        .set_match_game_id(id, req.match_index, req.game_id)
        .await;
    if !ok {
        return Err(StatusCode::NOT_FOUND);
    }

    // Stamp the tournament's broadcast delay onto the game row so the public
    // spectator feed for this match is gated (defeats live-stream ghosting).
    if tournament.broadcast_delay_secs > 0 {
        let repo = GameRepository::new(state.store.pool());
        if let Err(e) = repo
            .set_broadcast_delay(
                &req.game_id.to_string(),
                tournament.broadcast_delay_secs as i64,
            )
            .await
        {
            error!(
                "[tournament] failed to set broadcast delay for game {}: {}",
                req.game_id, e
            );
        }
    }

    info!(
        "[tournament] Match {} of tournament {} assigned game_id {}",
        req.match_index, id, req.game_id
    );
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// POST /admin/tournament/:id/initialize-swiss - Initializes a Swiss tournament and starts round 1
async fn initialize_swiss_tournament(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = &state.tournament_store;
    let tournament = store.get(id).await.ok_or(StatusCode::NOT_FOUND)?;

    // Check if Swiss format
    let rounds = match tournament.format {
        TournamentFormat::Swiss { rounds } => rounds,
        TournamentFormat::SingleElimination => return Err(StatusCode::BAD_REQUEST),
    };

    // Check minimum players (a Swiss event needs at least one pairing)
    let current_players = tournament.players.len() as u16;
    let min_players = tournament.min_players.unwrap_or(2);
    if current_players < min_players {
        return Err(StatusCode::CONFLICT); // Not enough players
    }

    // Seed players by ELO
    let mut seeded_tournament = tournament.clone();
    seed_players_by_elo(&mut seeded_tournament);
    seeded_tournament.status = TournamentStatus::Active;
    seeded_tournament.started_at = Some(chrono::Utc::now().timestamp());

    // Initialize Swiss data
    seeded_tournament.swiss_data = Some(crate::signing::storage::tournament::SwissStorageData {
        current_round: 0,
        total_rounds: rounds,
        rounds: Vec::new(),
        results: Vec::new(),
        standings: Vec::new(),
        round_deadline_at: None,
        absent_players: Vec::new(),
        withdrawn_players: Vec::new(),
        forbidden_pairs: Vec::new(),
        manual_pairings_next_round: Vec::new(),
    });

    store.create(seeded_tournament).await;

    // Ensure gossip topic is registered for real-time updates
    state.tournament_gossip.ensure_topic_registered(id).await;

    if let Err(err) = state.swiss_service.start_round(id).await {
        error!(
            "[tournament] Failed to start Swiss round 1 for {}: {:?}",
            id, err
        );
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    info!(
        "[tournament] Swiss tournament {} initialized with {} players, {} rounds",
        id, current_players, rounds
    );

    Ok(Json(serde_json::json!({
        "ok": true,
        "tournament_id": id,
        "players": current_players,
        "rounds": rounds
    })))
}

/// POST /tournament/:id/join - Joins a tournament.
///
/// Enforces CACF KYC when `kyc_required` is set on the tournament.
/// Players must have completed identity registration via `/identity/register`
/// (on the website or in-game) before they can enter a KYC-gated tournament.
async fn join_tournament(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let player = body
        .get("player")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;
    let elo = body.get("elo").and_then(|v| v.as_u64()).unwrap_or(1200) as u32;

    let store = &state.tournament_store;

    // Load tournament early so we can check kyc_required before mutating state.
    let tournament = store.get(id).await.ok_or(StatusCode::NOT_FOUND)?;

    // ── CACF KYC gate ────────────────────────────────────────────────────────
    // When kyc_required is true every entrant must have a row in vault_users,
    // which is written by POST /identity/register (web or in-game flow).
    if tournament.kyc_required {
        let row = sqlx::query("SELECT 1 FROM vault_users WHERE pubkey = ?")
            .bind(player)
            .fetch_optional(&*state.vault_pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        if row.is_none() {
            info!(
                "[tournament] KYC gate rejected {} for tournament {} — CACF not completed",
                player, id
            );
            return Ok(Json(serde_json::json!({
                "ok": false,
                "kyc_rejected": true,
                "message": "CACF KYC verification required. Complete identity registration on the website or in-game."
            })));
        }
    }

    let mut slot = None;
    let mut just_full = false;
    let mut elo_rejected = false;
    let mut elo_min = None;
    let mut elo_max = None;

    let ok = store
        .update(id, |t| {
            // ELO filtering
            if let (Some(min), Some(max)) = (t.elo_min, t.elo_max) {
                if elo < min || elo > max {
                    elo_rejected = true;
                    elo_min = Some(min);
                    elo_max = Some(max);
                    return;
                }
            }

            if t.is_full() {
                return;
            }
            if t.players.iter().any(|p| p == player) {
                slot = Some(t.players.len());
                return;
            }
            slot = Some(t.players.len());
            t.players.push(player.to_string());
            t.player_elos.push(elo);
            // Prize contribution = entry_fee minus platform_fee (50p goes to treasury, £2.50 to pot)
            t.prize_pool += t.entry_fee_lamports.saturating_sub(t.platform_fee_lamports);
            // Check if tournament just filled
            if t.players.len() == t.max_players as usize {
                just_full = true;
            }
        })
        .await;

    if !ok {
        return Err(StatusCode::NOT_FOUND);
    }

    if elo_rejected {
        return Err(StatusCode::FORBIDDEN); // ELO out of range
    }

    let position = slot.ok_or(StatusCode::CONFLICT)?;

    // Send Braid scheduler trigger for auto-start logic
    if let Some(ref trigger_tx) = state.tournament_trigger {
        let player_count = store.get(id).await.map(|t| t.players.len()).unwrap_or(0);
        let trigger = TournamentTrigger::PlayerJoined {
            tournament_id: id,
            player_count,
        };
        if let Err(e) = trigger_tx.send(trigger).await {
            warn!("[tournament] Failed to send scheduler trigger: {}", e);
        } else {
            info!(
                "[tournament] Sent Braid scheduler trigger for tournament {} ({} players)",
                id, player_count
            );
        }
    }

    // Old auto-start logic replaced by Braid scheduler
    // Scheduler will handle bracket generation and tournament start based on format
    if just_full {
        info!(
            "[tournament] {} joined tournament {} at slot {} - FULL, scheduler will auto-start",
            player, id, position
        );
    } else {
        info!(
            "[tournament] {} joined tournament {} at slot {}/{}",
            player,
            id,
            position,
            store.get(id).await.map(|t| t.max_players).unwrap_or(0)
        );
    }

    Ok(Json(serde_json::json!({
        "ok": true,
        "slot": position,
        "tournament_full": just_full,
        "elo_rejected": elo_rejected,
        "elo_range": elo_min.zip(elo_max)
    })))
}

/// Request to build a leave transaction.
#[derive(Deserialize)]
pub struct BuildLeaveTxReq {
    pub player: String,
}

/// POST /tournament/:id/build-leave-tx - Builds a partially signed transaction to leave a tournament.
async fn build_leave_transaction(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<BuildLeaveTxReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = &state.tournament_store;
    let tournament = store.get(id).await.ok_or(StatusCode::NOT_FOUND)?;

    let player_pubkey = Pubkey::from_str(&req.player).map_err(|_| StatusCode::BAD_REQUEST)?;
    let program_id = Pubkey::from_str(&state.config.program_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // 1. Build the instruction (refund comes from the tournament escrow PDA;
    //    the player is the only signer)
    let instruction = crate::signing::solana::leave_tournament_ix(
        &program_id,
        id,
        tournament.max_players,
        &player_pubkey,
    );

    // 2. Fetch latest blockhash
    let rpc = crate::signing::solana::make_rpc(&state.config.solana_rpc_url);
    let blockhash = rpc.get_latest_blockhash().map_err(|e| {
        error!("[tournament] Failed to get blockhash: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // 3. Build the unsigned transaction — the player signs it client-side
    let message = Message::new(&[instruction], Some(&player_pubkey));
    let mut transaction = Transaction::new_unsigned(message);
    transaction.message.recent_blockhash = blockhash;

    // 4. Serialize to base64
    let tx_bytes =
        bincode::serialize(&transaction).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let transaction_base64 = BASE64_STANDARD.encode(&tx_bytes);

    info!(
        "[tournament] Built leave transaction for {} in tournament {}",
        req.player, id
    );

    Ok(Json(serde_json::json!({
        "ok": true,
        "transaction": transaction_base64,
    })))
}

/// POST /tournament/:id/leave - Removes a player from a tournament after tx is confirmed.
async fn leave_tournament(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<BuildLeaveTxReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = &state.tournament_store;

    // Attempt to leave
    let ok = store.leave_tournament(id, &req.player).await;
    if !ok {
        return Err(StatusCode::NOT_FOUND);
    }

    info!("[tournament] {} left tournament {}", req.player, id);

    Ok(Json(serde_json::json!({
        "ok": true,
        "player": req.player,
    })))
}

/// Seeds players by ELO descending (highest first).
/// Call before generating bracket.
fn seed_players_by_elo(t: &mut TournamentRecord) {
    let mut indexed: Vec<(usize, u32)> = t.player_elos.iter().copied().enumerate().collect();
    indexed.sort_by(|a, b| b.1.cmp(&a.1));

    let mut seeded_players = Vec::with_capacity(t.players.len());
    let mut seeded_elos = Vec::with_capacity(t.player_elos.len());

    for (idx, elo) in indexed {
        seeded_players.push(t.players[idx].clone());
        seeded_elos.push(elo);
    }

    t.players = seeded_players;
    t.player_elos = seeded_elos;
}

/// Request to build a fund prize transaction.
#[derive(Deserialize)]
pub struct FundPrizeReq {
    pub amount: u64,
}

/// Response with base64-encoded transaction.
#[derive(Serialize)]
pub struct FundPrizeRes {
    pub transaction: String,
    pub tournament_id: u64,
    pub amount: u64,
}

/// POST /admin/tournament/{id}/fund-prize-tx - Build transaction to fund USDC prize pool.
async fn build_fund_prize_transaction(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<FundPrizeReq>,
) -> Result<Json<FundPrizeRes>, StatusCode> {
    info!(
        "[tournament] Building fund prize tx for tournament {} amount {}",
        id, req.amount
    );

    // Verify tournament exists
    state
        .tournament_store
        .get(id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    // Build transaction (simplified - actual implementation would use solana_sdk)
    let transaction_base64 = "placeholder".to_string();

    Ok(Json(FundPrizeRes {
        transaction: transaction_base64,
        tournament_id: id,
        amount: req.amount,
    }))
}

/// POST /admin/tournament/{id}/cancel - Build transaction to cancel tournament.
async fn build_cancel_transaction(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("[tournament] Building cancel tx for tournament {}", id);

    // Verify tournament exists
    let tournament = state
        .tournament_store
        .get(id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(serde_json::json!({
        "transaction": "placeholder",
        "tournament_id": id,
        "players_to_refund": tournament.players.len(),
    })))
}

/// GET /admin/tournament/:id/gossip-status - Check if gossip topic is registered.
async fn get_gossip_status(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let has_topic = state.tournament_gossip.has_topic(id).await;
    let subscriber_count = state.tournament_gossip.get_subscriber_count(id).await;

    Ok(Json(serde_json::json!({
        "tournament_id": id,
        "has_topic": has_topic,
        "subscriber_count": subscriber_count,
    })))
}

/// Player-facing tournament routes (no admin auth required).
pub fn tournaments_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_tournaments))
        .route("/my", get(list_my_tournaments))
}

/// Creates gossip-enabled tournament routes (requires AppState).
pub fn tournament_gossip_routes() -> Router<AppState> {
    Router::new()
        .route("/{id}/subscribe-node", post(subscribe_node))
        .route("/{id}/bootstrap-peers", get(get_bootstrap_peers))
}

/// Core tournament interaction routes.
pub fn tournament_routes() -> Router<AppState> {
    Router::new()
        .route("/{id}", get(get_tournament))
        .route("/{id}/join", post(join_tournament))
        .route("/{id}/register-node", post(register_node))
        .route("/{id}/my-match", get(get_my_match))
        .route("/{id}/bracket", get(get_bracket))
        .route("/{id}/build-leave-tx", post(build_leave_transaction))
        .route("/{id}/leave", post(leave_tournament))
        .route("/{id}/schedule-status", get(get_schedule_status))
        .route(
            "/{id}/session-create-game",
            post(tournament_session_create_game),
        )
        .route(
            "/{id}/session-join-game",
            post(tournament_session_join_game),
        )
        .merge(tournament_gossip_routes())
}

/// Admin-only tournament management routes.
/// POST /admin/tournament/:id/set-round-deadline — set deadline Unix timestamp for the current round.
#[derive(Deserialize)]
struct SetRoundDeadlineReq {
    /// Unix timestamp (seconds) when the round must end. Pass null to clear.
    deadline_at: Option<i64>,
}

async fn set_round_deadline(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<SetRoundDeadlineReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let updated = state
        .tournament_store
        .update(id, |t| {
            if let Some(ref mut swiss) = t.swiss_data {
                swiss.round_deadline_at = req.deadline_at;
            }
        })
        .await;
    if !updated {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// POST /admin/tournament/:id/import-players-csv — bulk-import players from CSV body.
/// CSV format: one wallet address per line (or comma-separated), skips empty lines.
async fn import_players_csv(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    body: axum::body::Bytes,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let csv_text = std::str::from_utf8(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
    let players: Vec<String> = csv_text
        .lines()
        .flat_map(|line| line.split(','))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if players.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut results: Vec<serde_json::Value> = Vec::new();
    let updated = state
        .tournament_store
        .update(id, |t| {
            for p in &players {
                if !t.players.contains(p) {
                    t.players.push(p.clone());
                    results.push(serde_json::json!({ "player": p, "status": "added" }));
                } else {
                    results
                        .push(serde_json::json!({ "player": p, "status": "already_registered" }));
                }
            }
        })
        .await;

    if !updated {
        return Err(StatusCode::NOT_FOUND);
    }
    info!(
        "[tournament] Bulk CSV import: {} players processed for tournament {}",
        players.len(),
        id
    );
    Ok(Json(serde_json::json!({ "ok": true, "results": results })))
}

pub fn admin_tournament_routes() -> Router<AppState> {
    Router::new()
        .route("/create", post(create_tournament))
        .route("/{id}/record-result", post(record_result))
        .route("/{id}/set-match-game-id", post(set_match_game_id))
        .route("/{id}/initialize-swiss", post(initialize_swiss_tournament))
        .route("/{id}/fund-prize-tx", post(build_fund_prize_transaction))
        .route("/{id}/cancel", post(build_cancel_transaction))
        .route("/{id}/gossip-status", get(get_gossip_status))
        .route("/{id}/advance-round", post(advance_round))
        .route("/{id}/reseed", post(reseed_players))
        .route("/{id}/set-round-deadline", post(set_round_deadline))
        .route("/{id}/import-players-csv", post(import_players_csv))
}

// ── Item 7: Tournament session routing ───────────────────────────────────────

#[derive(Deserialize)]
struct TournamentSessionReq {
    /// The game_id assigned to this tournament match.
    game_id: u64,
    /// The creating player's wallet pubkey.
    wallet_pubkey: String,
}

#[derive(Serialize)]
struct TournamentSessionResp {
    /// The ephemeral session public key that was created for this game.
    session_pubkey: String,
}

/// POST /tournament/:id/session-create-game
/// Creates a VPS session for the white (creator) side of a tournament match game.
/// Idempotent: returns the existing session pubkey if already created.
async fn tournament_session_create_game(
    Path(tournament_id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<TournamentSessionReq>,
) -> Result<Json<TournamentSessionResp>, StatusCode> {
    let wallet = Pubkey::from_str(&req.wallet_pubkey).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Verify the player has a match in this tournament.
    let store = &state.tournament_store;
    let t = store
        .get(tournament_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;
    if t.match_for_player(&req.wallet_pubkey).is_none() {
        warn!(
            "[TOURNAMENT] session-create-game: player {} has no match in tournament {}",
            req.wallet_pubkey, tournament_id
        );
        return Err(StatusCode::PRECONDITION_FAILED);
    }

    let session_pubkey = state.store.create(req.game_id, wallet).await.map_err(|e| {
        error!(
            "[TOURNAMENT] create session for game {}: {}",
            req.game_id, e
        );
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    info!(
        "[TOURNAMENT] session-create-game: tournament {} game {} session {}",
        tournament_id, req.game_id, session_pubkey
    );
    Ok(Json(TournamentSessionResp {
        session_pubkey: session_pubkey.to_string(),
    }))
}

/// POST /tournament/:id/session-join-game
/// Creates/retrieves the VPS session for the black (joiner) side of a tournament match.
/// The session keypair was created by white's call to session-create-game;
/// this call returns the same session pubkey so the joiner can include it in join_game.
async fn tournament_session_join_game(
    Path(tournament_id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<TournamentSessionReq>,
) -> Result<Json<TournamentSessionResp>, StatusCode> {
    let wallet = Pubkey::from_str(&req.wallet_pubkey).map_err(|_| StatusCode::BAD_REQUEST)?;

    let t = state
        .tournament_store
        .get(tournament_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;
    if t.match_for_player(&req.wallet_pubkey).is_none() {
        warn!(
            "[TOURNAMENT] session-join-game: player {} has no match in tournament {}",
            req.wallet_pubkey, tournament_id
        );
        return Err(StatusCode::PRECONDITION_FAILED);
    }

    // get-or-create: the session key is shared between both players.
    let session_pubkey = state.store.create(req.game_id, wallet).await.map_err(|e| {
        error!("[TOURNAMENT] join session for game {}: {}", req.game_id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    info!(
        "[TOURNAMENT] session-join-game: tournament {} game {} session {}",
        tournament_id, req.game_id, session_pubkey
    );
    Ok(Json(TournamentSessionResp {
        session_pubkey: session_pubkey.to_string(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_tournament_req_serialization() {
        let req = CreateTournamentReq {
            tournament_id: 1,
            name: "Test Tournament".to_string(),
            entry_fee_lamports: Some(1000000),
            platform_fee_lamports: None,
            max_players: 16,
            format: "SingleElimination".to_string(),
            swiss_rounds: None,
            elo_min: None,
            elo_max: None,
            min_players: None,
            prize_shares: Some([6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0]),
            winner_takes_all: false,
            scheduled_at: None,
            kyc_required: false,
        };

        let json = serde_json::to_string(&req);
        assert!(json.is_ok());
    }

    #[test]
    fn test_register_node_req_serialization() {
        let req = RegisterNodeReq {
            player: "test_wallet".to_string(),
            node_id: "node_123".to_string(),
        };

        let json = serde_json::to_string(&req);
        assert!(json.is_ok());
    }

    #[test]
    fn test_subscribe_node_req_serialization() {
        let req = SubscribeNodeReq {
            player: "test_wallet".to_string(),
            node_id: "node_123".to_string(),
        };

        let json = serde_json::to_string(&req);
        assert!(json.is_ok());
    }

    #[test]
    fn test_subscribe_node_res_serialization() {
        let res = SubscribeNodeRes {
            ok: true,
            bootstrap_peers: vec!["peer1".to_string(), "peer2".to_string()],
            topic_url: "/swiss/123".to_string(),
        };

        let json = serde_json::to_string(&res);
        assert!(json.is_ok());
    }

    #[test]
    fn test_record_result_req_serialization() {
        let req = RecordResultReq {
            match_index: 0,
            winner: "player1".to_string(),
            loser: "player2".to_string(),
            reason: None,
        };

        let json = serde_json::to_string(&req);
        assert!(json.is_ok());
    }

    #[test]
    fn test_set_match_game_id_req_serialization() {
        let req = SetMatchGameIdReq {
            match_index: 0,
            game_id: 12345,
        };

        let json = serde_json::to_string(&req);
        assert!(json.is_ok());
    }

    #[test]
    fn test_tournament_summary_serialization() {
        let summary = TournamentSummary {
            tournament_id: 1,
            name: "Test Tournament".to_string(),
            entry_fee_lamports: 1000000,
            prize_pool: 16000000,
            max_players: 16,
            registered: 8,
            status: "Active".to_string(),
        };

        let json = serde_json::to_string(&summary);
        assert!(json.is_ok());
    }

    #[tokio::test]
    async fn test_tournament_routes_creation() {
        let _router = tournament_routes();
    }

    #[tokio::test]
    async fn test_admin_tournament_routes_creation() {
        let _router = admin_tournament_routes();
    }

    #[tokio::test]
    async fn test_tournaments_routes_creation() {
        let _router = tournaments_routes();
    }

    #[tokio::test]
    async fn test_tournament_gossip_routes_creation() {
        // Just verify the function signature compiles
        fn assert_gossip_routes<F>()
        where
            F: Fn() -> Router<AppState>,
        {
        }
        assert_gossip_routes::<fn() -> Router<AppState>>();
    }

    #[test]
    fn test_max_players_validation() {
        // Test valid player counts (power of 2)
        let valid_counts = vec![2, 4, 8, 16, 32, 64, 128];
        for count in valid_counts {
            let req = CreateTournamentReq {
                tournament_id: 1,
                name: "Test Tournament".to_string(),
                entry_fee_lamports: Some(1000000),
                platform_fee_lamports: None,
                max_players: count,
                format: "SingleElimination".to_string(),
                swiss_rounds: None,
                elo_min: None,
                elo_max: None,
                min_players: None,
                prize_shares: None,
                winner_takes_all: false,
                scheduled_at: None,
                kyc_required: false,
            };
            assert_eq!(req.max_players, count);
        }
    }

    #[test]
    fn test_prize_shares_default() {
        // Test default prize shares for different player counts
        let req_large = CreateTournamentReq {
            tournament_id: 1,
            name: "Test Tournament".to_string(),
            entry_fee_lamports: Some(1000000),
            platform_fee_lamports: None,
            max_players: 16,
            format: "SingleElimination".to_string(),
            swiss_rounds: None,
            elo_min: None,
            elo_max: None,
            min_players: None,
            prize_shares: None,
            winner_takes_all: false,
            scheduled_at: None,
            kyc_required: false,
        };

        let default_shares = if req_large.winner_takes_all {
            [10000, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        } else {
            match req_large.max_players {
                0..=64 => [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0],
                128 => [5000, 2500, 1500, 500, 500, 0, 0, 0, 0, 0],
                256 => [4000, 2000, 1200, 800, 600, 400, 300, 200, 200, 300],
                _ => [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0],
            }
        };

        let prize_shares = req_large.prize_shares.unwrap_or(default_shares);

        assert_eq!(prize_shares, [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn test_prize_shares_winner_take_all() {
        // Test winner-take-all for small tournaments
        let req_small = CreateTournamentReq {
            tournament_id: 1,
            name: "Test Tournament".to_string(),
            entry_fee_lamports: Some(1000000),
            platform_fee_lamports: None,
            max_players: 8,
            format: "SingleElimination".to_string(),
            swiss_rounds: None,
            elo_min: None,
            elo_max: None,
            min_players: None,
            prize_shares: None,
            winner_takes_all: true,
            scheduled_at: None,
            kyc_required: false,
        };

        let default_shares = if req_small.winner_takes_all {
            [10000, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        } else {
            match req_small.max_players {
                0..=64 => [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0],
                128 => [5000, 2500, 1500, 500, 500, 0, 0, 0, 0, 0],
                256 => [4000, 2000, 1200, 800, 600, 400, 300, 200, 200, 300],
                _ => [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0],
            }
        };

        let prize_shares = req_small.prize_shares.unwrap_or(default_shares);

        assert_eq!(prize_shares, [10000, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn test_entry_fee_lamports_validation() {
        // Test reasonable entry fee ranges
        let valid_fees = vec![0u64, 1000000, 5000000, 10000000];
        for fee in valid_fees {
            let req = CreateTournamentReq {
                tournament_id: 1,
                name: "Test Tournament".to_string(),
                entry_fee_lamports: Some(fee),
                platform_fee_lamports: None,
                max_players: 16,
                format: "SingleElimination".to_string(),
                swiss_rounds: None,
                elo_min: None,
                elo_max: None,
                min_players: None,
                prize_shares: None,
                winner_takes_all: false,
                scheduled_at: None,
                kyc_required: false,
            };
            assert_eq!(req.entry_fee_lamports, Some(fee));
        }
    }

    #[test]
    fn test_tournament_id_validation() {
        // Test valid tournament IDs
        let valid_ids = vec![0, 1, 100, u64::MAX];
        for id in valid_ids {
            let req = CreateTournamentReq {
                tournament_id: id,
                name: "Test Tournament".to_string(),
                entry_fee_lamports: Some(1000000),
                platform_fee_lamports: None,
                max_players: 16,
                format: "SingleElimination".to_string(),
                swiss_rounds: None,
                elo_min: None,
                elo_max: None,
                min_players: None,
                prize_shares: None,
                winner_takes_all: false,
                scheduled_at: None,
                kyc_required: false,
            };
            assert_eq!(req.tournament_id, id);
        }
    }

    #[test]
    fn test_node_id_format() {
        // Test that node ID is a non-empty string
        let req = RegisterNodeReq {
            player: "test_wallet".to_string(),
            node_id: "node_123".to_string(),
        };
        assert!(!req.node_id.is_empty());
    }

    #[test]
    fn test_match_index_validation() {
        // Test valid match indices
        let valid_indices = vec![0, 1, 10, 100];
        for index in valid_indices {
            let req = RecordResultReq {
                match_index: index,
                winner: "player1".to_string(),
                loser: "player2".to_string(),
                reason: None,
            };
            assert_eq!(req.match_index, index);
        }
    }

    #[test]
    fn test_game_id_validation() {
        // Test valid game IDs
        let valid_ids = vec![0, 1, 12345, u64::MAX];
        for id in valid_ids {
            let req = SetMatchGameIdReq {
                match_index: 0,
                game_id: id,
            };
            assert_eq!(req.game_id, id);
        }
    }

    #[test]
    fn test_seed_players_by_elo() {
        // Test ELO seeding function
        let mut record = TournamentRecord::with_config(
            1,
            "Test Tournament".to_string(),
            1000000,
            0,
            16,
            [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0],
            TournamentFormat::SingleElimination,
            None,
            None,
            None,
            None,
            false,
        );

        record.players = vec![
            "player1".to_string(),
            "player2".to_string(),
            "player3".to_string(),
            "player4".to_string(),
        ];
        record.player_elos = vec![1500, 2000, 1200, 1800];

        seed_players_by_elo(&mut record);

        // After seeding, should be sorted by ELO descending
        assert_eq!(record.player_elos[0], 2000); // Highest ELO first
        assert_eq!(record.player_elos[1], 1800);
        assert_eq!(record.player_elos[2], 1500);
        assert_eq!(record.player_elos[3], 1200); // Lowest ELO last
    }
}

// ── Schedule Status ─────────────────────────────────────────────────────

/// Response for `GET /tournament/:id/schedule-status`.
#[derive(serde::Serialize)]
struct ScheduleStatusResponse {
    phase: String,
    seconds_until_start: Option<i64>,
    current_players: usize,
    min_players: u16,
    max_players: u16,
    my_session_authorized: Option<bool>,
}

async fn get_schedule_status(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<Json<ScheduleStatusResponse>, StatusCode> {
    let tournament = state
        .tournament_store
        .get(id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    let now = chrono::Utc::now().timestamp();
    let scheduled_at = tournament.scheduled_at.unwrap_or(0);

    let phase = match tournament.status {
        TournamentStatus::Registration if scheduled_at > 0 && now < scheduled_at => {
            "countdown".to_string()
        }
        TournamentStatus::Registration if scheduled_at > 0 && now >= scheduled_at => {
            "grace_period".to_string()
        }
        TournamentStatus::Active => "active".to_string(),
        TournamentStatus::Completed => "completed".to_string(),
        TournamentStatus::Cancelled => "cancelled".to_string(),
        _ => "unknown".to_string(),
    };

    let seconds_until_start = if scheduled_at > 0 && now < scheduled_at {
        Some(scheduled_at - now)
    } else {
        None
    };

    let min_players = tournament.min_players.unwrap_or(8);

    Ok(Json(ScheduleStatusResponse {
        phase,
        seconds_until_start,
        current_players: tournament.players.len(),
        min_players,
        max_players: tournament.max_players,
        my_session_authorized: None, // populated by client from wallet state
    }))
}
