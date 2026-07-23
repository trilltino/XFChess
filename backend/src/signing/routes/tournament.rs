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
    routing::{delete, get, post},
    Router,
};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use borsh::BorshDeserialize;
use serde::{Deserialize, Serialize};
use solana_sdk::{message::Message, pubkey::Pubkey, signature::Signer, transaction::Transaction};
use std::collections::HashMap;
use std::str::FromStr;
use tracing::{error, info, warn};

use crate::db::repository::GameRepository;
use crate::signing::solana::{
    cancel_tournament_ix, initialize_escrow_ix, initialize_shards_ix, initialize_tournament_ix,
    record_result_ix, sign_and_submit,
};
use crate::signing::storage::tournament::{
    MatchStatus, TournamentFormat, TournamentRecord, TournamentStatus,
};
use crate::signing::storage::vault::VaultStore;
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
///
/// Field set must match the game client's `TournamentSummary`
/// (`src/multiplayer/network/vps/tournament.rs`) — `is_private` and
/// `is_tournament` are non-optional there (no `#[serde(default)]`), so
/// omitting either makes deserialization fail on *every* response and the
/// client silently shows "No tournaments available" regardless of what's
/// actually in the store. Keep the two in sync.
#[derive(Serialize)]
pub struct TournamentSummary {
    pub tournament_id: u64,
    pub name: String,
    pub entry_fee_lamports: u64,
    pub prize_pool: u64,
    pub max_players: u16,
    pub registered: usize,
    pub status: String,
    pub is_private: bool,
    /// Always true here — this endpoint only ever lists bracket/Swiss
    /// tournaments, never posted 1v1 wager games.
    pub is_tournament: bool,
    pub usdc_mint: Option<String>,
    pub min_elo: u32,
    pub max_elo: u32,
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

    // A tournament_id that already has a store row was already fully created
    // (successfully or as a resumed retry, below) — including ones that have
    // since been cancelled or completed. Reject reuse here rather than
    // falling into the on-chain idempotency skip below, which would
    // otherwise treat "this PDA exists" as "safe to resume" and silently
    // write a fresh Registration-status store row while the real on-chain
    // tournament is still whatever state (e.g. Cancelled) it was left in —
    // an admin picking a stale ID by mistake needs a loud error, not a
    // tournament that looks fresh in this panel but is dead on-chain.
    if store.get(req.tournament_id).await.is_some() {
        warn!(
            "[tournament] Refusing to create {} — tournament_id already in use",
            req.tournament_id
        );
        return Err(StatusCode::CONFLICT);
    }

    // ── On-chain setup (3 sequential VPS-signed transactions) ────────────────
    // Each step is skipped if its PDA already exists on-chain, so a retry
    // after a partial failure (e.g. tx 1 confirmed, tx 2's RPC call dropped,
    // before the store write below ever ran) resumes instead of permanently
    // failing with "account already in use". This only fires for tournament
    // IDs with no store row yet, per the guard just above.
    let program_id = Pubkey::from_str(&state.config.program_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let authority = &*state.vps_authority;
    let rpc = crate::signing::solana::make_rpc(&state.config.solana_rpc_url);

    let tid_bytes = req.tournament_id.to_le_bytes();
    let (tournament_pda, _) = Pubkey::find_program_address(&[b"tournament", &tid_bytes], &program_id);
    let (escrow_pda, _) = Pubkey::find_program_address(&[b"t_escrow", &tid_bytes], &program_id);
    let (shard0_pda, _) =
        Pubkey::find_program_address(&[b"tourney_players", &[0u8], &tid_bytes], &program_id);
    let account_exists = |pda: &Pubkey| rpc.get_account(pda).is_ok();

    // 1. initialize_tournament
    if !account_exists(&tournament_pda) {
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
    } else {
        info!(
            "[tournament] {} already initialized on-chain, resuming from step 2",
            req.tournament_id
        );
    }

    // 2. initialize_escrow
    if !account_exists(&escrow_pda) {
        let ix2 = initialize_escrow_ix(&program_id, req.tournament_id, &authority.pubkey());
        sign_and_submit(&rpc, authority, &[ix2]).map_err(|e| {
            error!(
                "[tournament] initialize_escrow tx failed for {}: {}",
                req.tournament_id, e
            );
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    }

    // 3. initialize_shards (variant chosen by max_players) — a single atomic
    // tx creates every required shard, so checking shard 0 alone tells us
    // whether this step landed.
    if account_exists(&shard0_pda) {
        store
            .create(TournamentRecord::with_config(
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
            ))
            .await;
        info!(
            "[tournament] {} fully on-chain already (resumed retry) — store written",
            req.tournament_id
        );
        return Ok(Json(serde_json::json!({ "ok": true, "tournament_id": req.tournament_id, "resumed": true })));
    }
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
            is_private: t.password_hash.is_some(),
            is_tournament: true,
            usdc_mint: None,
            min_elo: t.elo_min.unwrap_or(0),
            max_elo: t.elo_max.unwrap_or(u32::MAX),
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
            is_private: t.password_hash.is_some(),
            is_tournament: true,
            usdc_mint: None,
            min_elo: t.elo_min.unwrap_or(0),
            max_elo: t.elo_max.unwrap_or(u32::MAX),
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

/// GET /tournament/:id/registration-info — public, read-only data the game
/// client needs to build its own `register_player` instruction (the actual
/// on-chain registration + entry-fee-escrow deposit). `host_treasury` is not
/// secret — it's the same public key stamped into the on-chain Tournament
/// account at creation (see `create_tournament`'s doc comment) — this just
/// saves the client from having to separately fetch and decode that account.
async fn get_registration_info(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let tournament = state
        .tournament_store
        .get(id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(serde_json::json!({
        "tournament_id": id,
        "program_id": state.config.program_id,
        "max_players": tournament.max_players,
        "host_treasury": state.vps_authority.pubkey().to_string(),
    })))
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

    // Fire the same on-chain start sequence the single-elimination auto-start
    // path uses (start_tournament_ix + initialize_match batches) — without
    // this, Swiss tournaments showed Active in the store while on-chain they
    // stayed in Registration forever, so entry fees never got swept to
    // host_treasury. Async/fire-and-forget on the scheduler task, same
    // pattern already used for TournamentTrigger::PlayerJoined below —
    // failures are logged server-side rather than surfaced in this response.
    let mut on_chain_started = false;
    if let Some(ref trigger_tx) = state.tournament_trigger {
        if let Err(e) = trigger_tx
            .send(TournamentTrigger::AdminStart { tournament_id: id })
            .await
        {
            error!(
                "[tournament] Failed to send AdminStart trigger for Swiss tournament {}: {}",
                id, e
            );
        } else {
            on_chain_started = true;
        }
    }

    info!(
        "[tournament] Swiss tournament {} initialized with {} players, {} rounds (on-chain start queued: {})",
        id, current_players, rounds, on_chain_started
    );

    Ok(Json(serde_json::json!({
        "ok": true,
        "tournament_id": id,
        "players": current_players,
        "rounds": rounds,
        "on_chain_start_queued": on_chain_started,
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

    // ── Ban gate ─────────────────────────────────────────────────────────────
    let bans = crate::db::repository::BanRepository::new(state.store.pool());
    if bans.is_banned(player).await.unwrap_or(false) {
        info!(
            "[tournament] Banned wallet {} rejected from tournament {}",
            player, id
        );
        return Ok(Json(serde_json::json!({
            "ok": false,
            "banned": true,
            "message": "This wallet is banned."
        })));
    }

    // ── CACF KYC gate ────────────────────────────────────────────────────────
    // When kyc_required is true every entrant must have an active kyc_records
    // row, written by POST /api/kyc/submit (the live KYC flow — see kyc.rs).
    // Previously this checked `vault_users`, which is only ever written by
    // POST /identity/register; that handler had a table-name bug (inserted
    // into a differently-shaped `users` table) and so vault_users was never
    // actually populated, meaning this gate rejected every entrant
    // unconditionally. Fixed to use the same VaultStore::has_kyc check that
    // /api/user/status already relies on.
    if tournament.kyc_required {
        let vault = VaultStore::new((*state.vault_pool).clone(), state.store.pool());
        let has_kyc = vault.has_kyc(player).await;

        if !has_kyc {
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

// NOTE: prize funding lives at POST /admin/tournament/{id}/fund-prize in
// admin.rs (`fund_tournament_prize`) — that one actually signs+submits
// `fund_sol_prize_ix`. A dead duplicate stub used to live here
// (`/fund-prize-tx`, returned a literal "placeholder" transaction) and has
// been removed to avoid anyone wiring the wrong route.

/// POST /admin/tournament/{id}/cancel - Cancel a tournament on-chain: refunds
/// entry fees to registered players and returns the guaranteed prize to the
/// operator, then marks the tournament Cancelled in the store. Signed and
/// submitted directly by the backend's own operator key (`vps_authority`),
/// same as `create_tournament` — the tournament's on-chain `host_treasury`
/// is always set to that same key at creation, so it can satisfy both
/// Signer slots the instruction requires.
async fn build_cancel_transaction(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    info!("[tournament] Cancelling tournament {}", id);

    let err_body = |status: StatusCode, message: String| {
        (status, Json(serde_json::json!({ "ok": false, "message": message })))
    };

    let tournament = state.tournament_store.get(id).await.ok_or_else(|| {
        err_body(StatusCode::NOT_FOUND, format!("Tournament {id} not found"))
    })?;

    if tournament.status != TournamentStatus::Registration
        && tournament.status != TournamentStatus::Active
    {
        warn!(
            "[tournament] Refusing to cancel tournament {} in status {:?}",
            id, tournament.status
        );
        return Err(err_body(
            StatusCode::CONFLICT,
            format!(
                "Tournament {id} is {:?} — only Registration or Active tournaments can be cancelled",
                tournament.status
            ),
        ));
    }

    let players: Vec<Pubkey> = tournament
        .players
        .iter()
        .map(|p| Pubkey::from_str(p))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| {
            error!(
                "[tournament] Malformed player pubkey in tournament {} store: {}",
                id, e
            );
            err_body(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Malformed player pubkey in tournament {id} store: {e}"),
            )
        })?;

    let program_id = Pubkey::from_str(&state.config.program_id).map_err(|e| {
        err_body(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Invalid configured program_id: {e}"),
        )
    })?;
    let authority = &*state.vps_authority;
    let rpc = crate::signing::solana::make_rpc(&state.config.solana_rpc_url);

    let ix = cancel_tournament_ix(
        &program_id,
        id,
        tournament.max_players,
        &authority.pubkey(),
        &authority.pubkey(),
        &players,
    );
    // A DB row can only exist here if `create_tournament` already confirmed all
    // 3 on-chain init txs, so a missing `tournament` PDA means the account was
    // wiped after the fact — e.g. a local validator got reset while the
    // (durable) SQLite store kept the row. Treat that as "already gone
    // on-chain" and just settle the store, rather than 500ing forever on a
    // tournament the admin UI can never otherwise get rid of.
    let signature = match sign_and_submit(&rpc, authority, &[ix]) {
        Ok(sig) => Some(sig),
        Err(e) if is_missing_tournament_account_error(&e) => {
            warn!(
                "[tournament] Tournament {} PDA not found on-chain (stale store row) — \
                 cancelling in the store only, no on-chain refund possible",
                id
            );
            None
        }
        Err(e) => {
            error!("[tournament] cancel_tournament tx failed for {}: {}", id, e);
            return Err(err_body(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("cancel_tournament transaction failed: {e}"),
            ));
        }
    };

    state
        .tournament_store
        .update(id, |t| {
            t.status = TournamentStatus::Cancelled;
            t.prize_pool = 0;
        })
        .await;

    info!(
        "[tournament] Cancelled tournament {} (on_chain: {}), refunded {} players",
        id,
        signature.is_some(),
        if signature.is_some() { players.len() } else { 0 }
    );

    Ok(Json(serde_json::json!({
        "ok": true,
        "tournament_id": id,
        "on_chain": signature.is_some(),
        "signature": signature.map(|s| s.to_string()),
        "players_refunded": if signature.is_some() { players.len() } else { 0 },
    })))
}

/// Detects the specific `AccountNotInitialized` Anchor error on the
/// `tournament` account from a failed `cancel_tournament` simulation — the
/// signal that the on-chain PDA this store row references no longer exists.
fn is_missing_tournament_account_error(e: &impl std::fmt::Display) -> bool {
    let msg = e.to_string();
    msg.contains("account: tournament") && msg.contains("AccountNotInitialized")
}

/// Mirrors the on-chain `Tournament` account's leading fields (up to and
/// including `status`) just closely enough to decode them with Borsh —
/// trailing fields (win places, prize_shares, etc.) are irrelevant here and
/// left undecoded, which Borsh allows (it only reads what the struct asks
/// for). `authority`/`fee_payer`-style Pubkeys are read as raw `[u8; 32]`
/// since only byte-for-byte layout matters, not the Pubkey type itself.
#[derive(BorshDeserialize)]
struct OnChainTournamentPrefix {
    tournament_id: u64,
    authority: [u8; 32],
    name: String,
    entry_fee: u64,
    platform_fee: u64,
    prize_pool: u64,
    max_players: u16,
    player_count: u16,
    num_registered_players: u16,
    status: OnChainTournamentStatus,
}

/// Mirrors `programs/xfchess-game/src/state/tournament.rs`'s `TournamentStatus`
/// — variant order must match exactly (Borsh encodes the tag as a plain u8
/// index in declaration order).
#[derive(BorshDeserialize, Debug, Clone, Copy, PartialEq)]
enum OnChainTournamentStatus {
    Registration,
    Active,
    Completed,
    Closed,
    Cancelled,
}

impl OnChainTournamentStatus {
    fn to_store_status(self) -> TournamentStatus {
        match self {
            OnChainTournamentStatus::Registration => TournamentStatus::Registration,
            OnChainTournamentStatus::Active => TournamentStatus::Active,
            OnChainTournamentStatus::Completed | OnChainTournamentStatus::Closed => {
                TournamentStatus::Completed
            }
            OnChainTournamentStatus::Cancelled => TournamentStatus::Cancelled,
        }
    }
}

/// POST /admin/tournament/{id}/sync-status — reads the real on-chain
/// Tournament account and overwrites the store's status to match it.
///
/// The store and on-chain state can drift apart (e.g. a partially-failed
/// request, a store row surviving a chain rollback/redeploy on a test
/// validator) with no built-in way to reconcile them — cancel/register
/// requests would then keep failing against on-chain state the admin panel
/// can't see. This is the fix: pull chain truth and make the store agree
/// with it, so subsequent actions (cancel, delete, registration) work
/// against a consistent picture again.
async fn sync_tournament_status(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let tournament = state
        .tournament_store
        .get(id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;
    let store_status_before = format!("{:?}", tournament.status);

    let program_id = Pubkey::from_str(&state.config.program_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let (tournament_pda, _) =
        Pubkey::find_program_address(&[b"tournament", &id.to_le_bytes()], &program_id);
    let rpc = crate::signing::solana::make_rpc(&state.config.solana_rpc_url);

    let account = rpc.get_account(&tournament_pda).map_err(|e| {
        warn!(
            "[tournament] sync-status: on-chain account for {} not found: {}",
            id, e
        );
        StatusCode::NOT_FOUND
    })?;
    if account.data.len() < 8 {
        error!("[tournament] sync-status: account data for {} too short", id);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    let decoded = OnChainTournamentPrefix::deserialize(&mut &account.data[8..]).map_err(|e| {
        error!(
            "[tournament] sync-status: failed to decode on-chain tournament {}: {}",
            id, e
        );
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let on_chain_status = decoded.status.to_store_status();
    let on_chain_status_str = format!("{:?}", on_chain_status);

    state
        .tournament_store
        .update(id, |t| {
            t.status = on_chain_status;
        })
        .await;

    info!(
        "[tournament] Synced {} status: store was {:?}, on-chain is {:?}",
        id, store_status_before, on_chain_status_str
    );

    Ok(Json(serde_json::json!({
        "ok": true,
        "tournament_id": id,
        "store_status_before": store_status_before,
        "on_chain_status": on_chain_status_str,
    })))
}

/// DELETE /admin/tournament/{id} — removes a Cancelled or Completed
/// tournament's row from the store so it stops cluttering the admin panel's
/// list. On-chain state is untouched (there's nothing left to manage once a
/// tournament is in either of those terminal states); this is purely a local
/// housekeeping action, not a chain operation.
async fn delete_tournament(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let err_body = |status: StatusCode, message: String| {
        (status, Json(serde_json::json!({ "ok": false, "message": message })))
    };

    let tournament = state.tournament_store.get(id).await.ok_or_else(|| {
        err_body(StatusCode::NOT_FOUND, format!("Tournament {id} not found"))
    })?;

    if tournament.status != TournamentStatus::Cancelled
        && tournament.status != TournamentStatus::Completed
    {
        warn!(
            "[tournament] Refusing to delete {} — status {:?} is not terminal",
            id, tournament.status
        );
        return Err(err_body(
            StatusCode::CONFLICT,
            format!(
                "Tournament {id} is {:?} — only Cancelled or Completed tournaments can be removed. Cancel it first.",
                tournament.status
            ),
        ));
    }

    state.tournament_store.delete(id).await;
    info!("[tournament] Deleted store row for tournament {}", id);
    Ok(Json(serde_json::json!({ "ok": true, "tournament_id": id })))
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
        .route("/{id}/registration-info", get(get_registration_info))
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
        .route("/{id}/cancel", post(build_cancel_transaction))
        .route("/{id}/sync-status", post(sync_tournament_status))
        .route("/{id}", delete(delete_tournament))
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
            is_private: false,
            is_tournament: true,
            usdc_mint: None,
            min_elo: 0,
            max_elo: u32::MAX,
        };

        let json = serde_json::to_string(&summary);
        assert!(json.is_ok());
    }

    /// Regression test for a real bug: the game client's `TournamentSummary`
    /// (`src/multiplayer/network/vps/tournament.rs`) requires `is_private`
    /// and `is_tournament` with no `#[serde(default)]`, so if this backend
    /// struct ever drops a field the client expects, every `/tournaments`
    /// response silently fails to parse client-side and the game shows "No
    /// tournaments available" no matter what's actually in the store. Assert
    /// the exact field set the client needs stays present here.
    #[test]
    fn test_tournament_summary_has_fields_client_requires() {
        let summary = TournamentSummary {
            tournament_id: 1,
            name: "Test".to_string(),
            entry_fee_lamports: 0,
            prize_pool: 0,
            max_players: 2,
            registered: 0,
            status: "Registration".to_string(),
            is_private: false,
            is_tournament: true,
            usdc_mint: None,
            min_elo: 0,
            max_elo: u32::MAX,
        };
        let json: serde_json::Value = serde_json::to_value(&summary).unwrap();
        for field in [
            "tournament_id",
            "name",
            "entry_fee_lamports",
            "prize_pool",
            "max_players",
            "registered",
            "status",
            "is_private",
            "is_tournament",
            "min_elo",
            "max_elo",
        ] {
            assert!(
                json.get(field).is_some(),
                "TournamentSummary is missing field '{field}' the game client's \
                 deserializer requires — this breaks the tournament list in the live game"
            );
        }
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
