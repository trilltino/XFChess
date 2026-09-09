use bevy::prelude::*;
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use tokio::sync::oneshot;

use crate::multiplayer::solana::integration::state::DEVNET_RPC_URL;
use crate::solana::instructions::{
    accept_draw_ix, authorize_session_key_ix, claim_timeout_ix, create_game_ix, join_game_ix,
    offer_draw_ix, GAME_SEED, PROGRAM_ID as SOLANA_PROGRAM_ID,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LobbyMode {
    #[default]
    Create,
    Join,
    Browse,
    Tournament,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EloMatchPref {
    Strict, // ±50 ELO
    #[default]
    Expanded, // ±150 ELO
    Any,    // no filter
}

impl EloMatchPref {
    pub fn label(self) -> &'static str {
        match self {
            Self::Strict => "Strict ±50",
            Self::Expanded => "Normal ±150",
            Self::Any => "Any ELO",
        }
    }

    pub fn range(self) -> Option<u16> {
        match self {
            Self::Strict => Some(50),
            Self::Expanded => Some(150),
            Self::Any => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum LobbyStatus {
    Idle,
    Pending,
    Success(u64),
    Fetched {
        wager_sol: f64,
        game_id: u64,
    },
    WaitingForOpponent {
        game_id: u64,
    },
    OpponentJoined {
        game_id: u64,
    },
    WaitingForHostStart {
        game_id: u64,
    },
    EnterGame {
        game_id: u64,
    },
    Cancelling {
        game_id: u64,
    },
    Cancelled {
        game_id: u64,
        refunded: bool,
        message: String,
    },
    CancelFailed {
        game_id: u64,
        error: String,
    },
    Error(String),
}

impl Default for LobbyStatus {
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Resource)]
pub struct SolanaLobbyState {
    pub mode: LobbyMode,
    pub allow_create: bool,
    pub wager_sol: f32,
    pub wager_amount_input: String,
    pub match_type: u8,
    pub game_id_input: String,
    pub status: LobbyStatus,
    pub tx_rx: Option<oneshot::Receiver<Result<u64, String>>>,
    pub lookup_rx: Option<oneshot::Receiver<Result<(u64, u64), String>>>,
    pub opponent_poll_rx: Option<oneshot::Receiver<Result<(), String>>>,
    pub game_start_poll_rx: Option<oneshot::Receiver<Result<(), String>>>,
    pub cancel_rx: Option<oneshot::Receiver<Result<CancelOutcome, String>>>,
    // Cached from SolanaIntegrationState each frame.
    pub cached_balance: f64,
    pub cached_keypair_bytes: Option<Vec<u8>>,
    pub cached_rpc_url: String,
    pub cached_global_session_keypair_bytes: Option<Vec<u8>>,
    pub last_attempt_used_global_session: bool,
    pub cached_display_name: Option<String>,
    pub cached_node_id: Option<String>,
    pub cached_secret_key_bytes: Option<[u8; 32]>,
    pub cached_elo: u16,
    pub cached_region: Option<String>,
    pub room_password: Option<String>,
    pub time_control_base: u32,
    pub time_control_inc: u32,
    pub elo_pref: EloMatchPref,
    pub rejoin_rx: Option<oneshot::Receiver<Option<u64>>>,
    pub rejoin_game_id: Option<u64>,
    pub browse_games: Vec<crate::multiplayer::network::p2p_vps::VpsGameListing>,
    pub browse_last_fetch: Option<std::time::Instant>,
    pub browse_rx: Option<
        crossbeam_channel::Receiver<Vec<crate::multiplayer::network::p2p_vps::VpsGameListing>>,
    >,
    pub tournament_games: Vec<crate::multiplayer::network::vps::TournamentGameListing>,
    pub tournament_last_fetch: Option<std::time::Instant>,
    pub tournament_rx: Option<
        crossbeam_channel::Receiver<Vec<crate::multiplayer::network::vps::TournamentGameListing>>,
    >,
    pub announce_warning: Option<String>,
    pub last_lobby_heartbeat: Option<std::time::Instant>,
}

impl Default for SolanaLobbyState {
    fn default() -> Self {
        Self {
            mode: LobbyMode::default(),
            allow_create: true,
            wager_sol: 0.0,
            wager_amount_input: String::new(),
            match_type: 0,
            game_id_input: String::new(),
            status: LobbyStatus::default(),
            tx_rx: None,
            lookup_rx: None,
            opponent_poll_rx: None,
            game_start_poll_rx: None,
            cancel_rx: None,
            cached_balance: 0.0,
            cached_keypair_bytes: None,
            cached_rpc_url: DEVNET_RPC_URL.to_string(),
            cached_global_session_keypair_bytes: None,
            last_attempt_used_global_session: false,
            cached_display_name: None,
            cached_node_id: None,
            cached_secret_key_bytes: None,
            cached_elo: 0,
            cached_region: None,
            room_password: None,
            time_control_base: 300,
            time_control_inc: 0,
            elo_pref: EloMatchPref::default(),
            rejoin_rx: None,
            rejoin_game_id: None,
            browse_games: Vec::new(),
            browse_last_fetch: None,
            browse_rx: None,
            tournament_games: Vec::new(),
            tournament_last_fetch: None,
            tournament_rx: None,
            announce_warning: None,
            last_lobby_heartbeat: None,
        }
    }
}

impl SolanaLobbyState {
    pub fn wager_lamports(&self) -> u64 {
        (self.wager_sol as f64 * 1_000_000_000.0) as u64
    }
}

pub struct SolanaLobbyPlugin;

impl Plugin for SolanaLobbyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SolanaLobbyState>()
            .init_resource::<crate::multiplayer::solana::addon::SolanaGameSync>()
            .init_resource::<crate::multiplayer::solana::addon::CompetitiveMatchState>()
            .add_systems(
                Update,
                (
                    sync_from_solana_state,
                    poll_lobby_tasks,
                    poll_rejoin_check,
                    poll_solana_browse,
                    poll_tournament_games,
                )
                    .chain(),
            );
    }
}

// ---------------------------------------------------------------------------
// Async helpers (called from UI via IoTaskPool / Tokio)
// ---------------------------------------------------------------------------

pub fn spawn_create_game(
    rpc_url: String,
    wallet_pubkey: Pubkey,
    wager_lamports: u64,
    match_type: u8,
    time_base: u32,
    time_inc: u32,
    global_session_keypair_bytes: Option<Vec<u8>>,
    tx: oneshot::Sender<Result<u64, String>>,
) {
    let program_id: solana_sdk::pubkey::Pubkey = SOLANA_PROGRAM_ID.parse().unwrap_or_default();

    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            let result = async_create_game(
                rpc_url,
                wallet_pubkey,
                program_id,
                wager_lamports,
                match_type,
                time_base,
                time_inc,
                global_session_keypair_bytes,
            )
            .await;
            if let Ok(game_id) = &result {
                if wager_lamports > 0 {
                    crate::multiplayer::solana::wager_recovery::record(*game_id);
                }
            }
            let _ = tx.send(result);
        })
        .detach();
}

pub fn spawn_offer_draw(rpc_url: String, wallet_pubkey: Pubkey, game_id: u64) {
    let program_id: Pubkey = SOLANA_PROGRAM_ID.parse().unwrap_or_default();
    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            if let Err(e) = async_offer_draw(rpc_url, wallet_pubkey, program_id, game_id).await {
                error!("[DRAW] offer_draw on-chain tx failed: {e}");
            }
        })
        .detach();
}

async fn async_offer_draw(
    rpc_url: String,
    wallet_pubkey: Pubkey,
    program_id: Pubkey,
    game_id: u64,
) -> Result<(), String> {
    use crate::multiplayer::solana::tauri_signer::sign_and_send_via_tauri;

    let ix = offer_draw_ix(program_id, game_id, wallet_pubkey)
        .map_err(|e| format!("build offer_draw_ix: {e}"))?;
    let sig = sign_and_send_via_tauri(&rpc_url, wallet_pubkey, &[ix], &[], "Offering draw")?;
    info!("[DRAW] offer_draw confirmed on-chain: {sig}");
    Ok(())
}

pub fn spawn_accept_draw(rpc_url: String, wallet_pubkey: Pubkey, game_id: u64) {
    let program_id: Pubkey = SOLANA_PROGRAM_ID.parse().unwrap_or_default();
    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            if let Err(e) = async_accept_draw(rpc_url, wallet_pubkey, program_id, game_id).await {
                error!("[DRAW] accept_draw on-chain tx failed: {e}");
            }
        })
        .detach();
}

async fn async_accept_draw(
    rpc_url: String,
    wallet_pubkey: Pubkey,
    program_id: Pubkey,
    game_id: u64,
) -> Result<(), String> {
    use crate::multiplayer::solana::tauri_signer::sign_and_send_via_tauri;

    let ix = accept_draw_ix(program_id, game_id, wallet_pubkey)
        .map_err(|e| format!("build accept_draw_ix: {e}"))?;
    let sig = sign_and_send_via_tauri(&rpc_url, wallet_pubkey, &[ix], &[], "Accepting draw")?;
    info!("[DRAW] accept_draw confirmed on-chain: {sig}");
    Ok(())
}

pub fn spawn_claim_timeout(rpc_url: String, wallet_pubkey: Pubkey, game_id: u64) {
    let program_id: Pubkey = SOLANA_PROGRAM_ID.parse().unwrap_or_default();
    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            if let Err(e) = async_claim_timeout(rpc_url, wallet_pubkey, program_id, game_id).await {
                error!("[TIMEOUT] claim_timeout on-chain tx failed: {e}");
            }
        })
        .detach();
}

async fn async_claim_timeout(
    rpc_url: String,
    wallet_pubkey: Pubkey,
    program_id: Pubkey,
    game_id: u64,
) -> Result<(), String> {
    use crate::multiplayer::solana::tauri_signer::sign_and_send_via_tauri;

    let ix = claim_timeout_ix(program_id, game_id, wallet_pubkey)
        .map_err(|e| format!("build claim_timeout_ix: {e}"))?;
    let sig = sign_and_send_via_tauri(&rpc_url, wallet_pubkey, &[ix], &[], "Claiming timeout")?;
    info!("[TIMEOUT] claim_timeout confirmed on-chain: {sig}");
    Ok(())
}

pub fn spawn_lookup_game(
    rpc_url: String,
    game_id: u64,
    tx: oneshot::Sender<Result<(u64, u64), String>>,
) {
    let program_id: solana_sdk::pubkey::Pubkey = SOLANA_PROGRAM_ID.parse().unwrap_or_default();

    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            let result = async_lookup_game(rpc_url, program_id, game_id).await;
            let _ = tx.send(result);
        })
        .detach();
}

pub fn spawn_poll_game_start(game_id: u64, tx: oneshot::Sender<Result<(), String>>) {
    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            let result = async_poll_game_start(game_id).await;
            let _ = tx.send(result);
        })
        .detach();
}

pub fn spawn_poll_opponent_joined(
    rpc_url: String,
    game_id: u64,
    tx: oneshot::Sender<Result<(), String>>,
) {
    let program_id: solana_sdk::pubkey::Pubkey = SOLANA_PROGRAM_ID.parse().unwrap_or_default();

    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            let result = async_poll_opponent_joined(rpc_url, program_id, game_id).await;
            let _ = tx.send(result);
        })
        .detach();
}

pub fn spawn_join_game(
    rpc_url: String,
    wallet_pubkey: Pubkey,
    game_id: u64,
    global_session_keypair_bytes: Option<Vec<u8>>,
    tx: oneshot::Sender<Result<u64, String>>,
) {
    let program_id: solana_sdk::pubkey::Pubkey = SOLANA_PROGRAM_ID.parse().unwrap_or_default();

    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            let result = async_join_game(
                rpc_url,
                wallet_pubkey,
                program_id,
                game_id,
                global_session_keypair_bytes,
            )
            .await;
            if let Ok(game_id) = &result {
                crate::multiplayer::solana::wager_recovery::record(*game_id);
            }
            let _ = tx.send(result);
        })
        .detach();
}

// ---------------------------------------------------------------------------
// Private async implementations
// ---------------------------------------------------------------------------

async fn async_poll_opponent_joined(
    rpc_url: String,
    program_id: solana_sdk::pubkey::Pubkey,
    game_id: u64,
) -> Result<(), String> {
    use std::time::{Duration, Instant};
    const POLL_INTERVAL: Duration = Duration::from_secs(3);
    const TIMEOUT: Duration = Duration::from_secs(300);
    const BLACK_OFFSET: usize = 8 + 8 + 32; // disc + game_id + white pubkey

    let rpc = solana_client::rpc_client::RpcClient::new_with_commitment(
        rpc_url,
        CommitmentConfig::confirmed(),
    );
    let game_pda = solana_sdk::pubkey::Pubkey::find_program_address(
        &[GAME_SEED, &game_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let start = Instant::now();
    loop {
        if start.elapsed() > TIMEOUT {
            return Err("Timed out waiting for opponent (5 min)".to_string());
        }

        match rpc.get_account_data(&game_pda) {
            Ok(data) if data.len() >= BLACK_OFFSET + 32 => {
                let black_bytes = &data[BLACK_OFFSET..BLACK_OFFSET + 32];
                let default_bytes = [0u8; 32];
                if black_bytes != default_bytes {
                    return Ok(());
                }
            }
            Ok(_) => {}
            Err(e) => {
                return Err(format!("poll RPC: {}", e));
            }
        }

        // Use blocking sleep inside IoTaskPool (it runs on a thread pool, not async executor)
        std::thread::sleep(POLL_INTERVAL);
    }
}

async fn async_poll_game_start(game_id: u64) -> Result<(), String> {
    use std::time::{Duration, Instant};
    const POLL_INTERVAL: Duration = Duration::from_secs(2);
    const TIMEOUT: Duration = Duration::from_secs(600);

    let target = game_id.to_string();
    let start = Instant::now();
    loop {
        if start.elapsed() > TIMEOUT {
            return Err("Timed out waiting for the host to start (10 min)".to_string());
        }

        match crate::multiplayer::vps_client::p2p_list_games() {
            Ok(games) => {
                if games
                    .iter()
                    .any(|g| g.game_id == target && g.status == "InProgress")
                {
                    return Ok(());
                }
            }
            Err(e) => warn!("[LOBBY] poll for host start failed: {}", e),
        }

        std::thread::sleep(POLL_INTERVAL);
    }
}

async fn async_create_game(
    rpc_url: String,
    wallet_pubkey: Pubkey,
    program_id: solana_sdk::pubkey::Pubkey,
    wager_lamports: u64,
    match_type: u8,
    time_base: u32,
    time_inc: u32,
    global_session_keypair_bytes: Option<Vec<u8>>,
) -> Result<u64, String> {
    use crate::multiplayer::solana::tauri_signer::sign_via_tauri_only;
    use crate::multiplayer::vps_client;
    use std::time::Instant;

    // Gate: only wallets with profile + email + KYC may create a wagered match.
    if wager_lamports > 0 {
        crate::multiplayer::network::vps::identity::require_wager_eligibility(
            &wallet_pubkey.to_string(),
        )?;
    }

    let game_id: u64 = rand::random();

    if let Some(kp_bytes) = global_session_keypair_bytes {
        return async_create_game_via_global_session(
            rpc_url,
            wallet_pubkey,
            program_id,
            game_id,
            wager_lamports,
            match_type,
            time_base,
            time_inc,
            kp_bytes,
        )
        .await;
    }

    // 1. Ask VPS to generate session keypair → get session_pubkey + platform fee.
    let legacy_start = Instant::now();
    info!(
        "[CREATE_GAME] legacy per-game signing path selected for game {game_id} (wager_lamports={wager_lamports}, match_type={match_type})"
    );

    let step_start = Instant::now();
    let (session_pubkey_str, platform_fee_lamports) =
        vps_client::create_session(game_id, &wallet_pubkey.to_string())?;
    info!(
        "[CREATE_GAME] /session/create completed for game {game_id} in {:?}",
        step_start.elapsed()
    );
    let session_pubkey: Pubkey = session_pubkey_str
        .parse()
        .map_err(|e| format!("parse session_pubkey: {e}"))?;

    let create_ix = create_game_ix(
        program_id,
        wallet_pubkey,
        session_pubkey,
        game_id,
        wager_lamports,
        match_type,
        platform_fee_lamports,
        time_base as u64,
        time_inc as u16,
    )
    .map_err(|e| format!("build create_game_ix: {e}"))?;
    let auth_ix =
        authorize_session_key_ix(program_id, wallet_pubkey, game_id, session_pubkey, 86400)
            .map_err(|e| format!("build authorize_session_key_ix: {e}"))?;

    let ixs = vec![create_ix, auth_ix];

    // 3. ONE wallet popup — signs everything together.
    let label = if wager_lamports > 0 {
        "Creating wagered game"
    } else {
        "Creating game"
    };
    let step_start = Instant::now();
    let signed_bytes = sign_via_tauri_only(&rpc_url, wallet_pubkey, &ixs, &[], label)
        .map_err(|e| format!("sign bundled TX: {e}"))?;
    info!(
        "[CREATE_GAME] wallet signing completed for game {game_id} in {:?}",
        step_start.elapsed()
    );

    // 4. VPS submits TX + funds session key (no more separate popups).
    let step_start = Instant::now();
    vps_client::activate_session(game_id, &signed_bytes)?;
    info!(
        "[CREATE_GAME] /session/activate completed for game {game_id} in {:?}",
        step_start.elapsed()
    );

    // `/session/activate` already waits for confirmed commitment server-side.
    // The additional client-side get-account re-poll was redundant and added
    // unnecessary latency to the create-game end-to-end path.
    info!(
        "[CREATE_GAME] legacy per-game create finished for game {game_id} in {:?}",
        legacy_start.elapsed()
    );

    Ok(game_id)
}

async fn async_create_game_via_global_session(
    rpc_url: String,
    wallet_pubkey: Pubkey,
    program_id: Pubkey,
    game_id: u64,
    wager_lamports: u64,
    match_type: u8,
    time_base: u32,
    time_inc: u32,
    session_keypair_bytes: Vec<u8>,
) -> Result<u64, String> {
    use crate::multiplayer::solana::global_session_manager::{
        build_global_create_game_ix, check_global_session_can_afford_wager, find_global_session_pda,
    };
    use crate::solana::instructions::WAGER_ESCROW_SEED;
    use solana_sdk::signature::{Keypair, Signer};
    use std::time::Instant;

    let session_kp = Keypair::try_from(session_keypair_bytes.as_slice())
        .map_err(|e| format!("session keypair: {e}"))?;
    let (session_pda, _bump) = find_global_session_pda(&program_id, &wallet_pubkey);
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;

    // Catch an under-funded quick-sign session before spending a transaction
    // fee on a doomed submit — the on-chain spending_limit/max_wager caps are
    // self-declared, not balance-aware, so they alone don't guarantee the
    // vault can actually cover this wager. See
    // `check_global_session_can_afford_wager`'s doc comment.
    check_global_session_can_afford_wager(&rpc_url, &session_pda, wager_lamports)?;

    // Mirrors the on-chain settlement gate (`match_type != MatchType::Free`,
    // see `lifecycle/settlement.rs`) — a Free match never gets charged the
    // platform fee at settlement regardless of what's passed here, so don't
    // bother fetching it (or failing game creation over a rate-fetch hiccup)
    // for a match type that will just discard it anyway.
    let platform_fee_lamports = if match_type != 0 {
        crate::multiplayer::vps_client::fetch_platform_fee_lamports()
            .map_err(|e| format!("fetch platform fee: {e}"))?
    } else {
        0
    };

    let ix = build_global_create_game_ix(
        &program_id,
        &session_pda,
        &session_kp.pubkey(),
        &wallet_pubkey,
        &game_pda,
        &escrow_pda,
        game_id,
        wager_lamports,
        match_type,
        platform_fee_lamports,
        time_base as u64,
        time_inc as u16,
    );

    let rpc = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());

    let start = Instant::now();
    crate::multiplayer::solana::submit::submit_local_tx(
        &rpc,
        &session_kp,
        &[ix],
        crate::multiplayer::solana::submit::SubmitConfig::fast(),
    )
    .map_err(|e| format!("global_create_game submit: {e}"))?;

    info!(
        "[CREATE_GAME] global_create_game landed for game {} in {:?} (session-signed, no wallet popup)",
        game_id,
        start.elapsed()
    );

    // Best-effort: lets settlement_worker discover this game — see
    // `track_global_session_game`'s doc comment. Never blocks success on
    // this; a failure here just means this one game isn't auto-settled.
    if let Err(e) = crate::multiplayer::vps_client::track_global_session_game(
        game_id,
        &wallet_pubkey.to_string(),
    ) {
        warn!("[CREATE_GAME] track_global_session_game failed for {game_id}: {e}");
    }

    Ok(game_id)
}

#[derive(Debug)]
pub enum CancelOutcome {
    Refunded(solana_sdk::signature::Signature),
    NothingToRefund(String),
}

fn is_user_rejection(e: &str) -> bool {
    let l = e.to_lowercase();
    l.contains("reject")
        || l.contains("denied")
        || l.contains("declined")
        || l.contains("user cancel")
        || l.contains("user closed")
}

pub fn cancel_game_on_chain(
    rpc_url: String,
    program_id: Pubkey,
    wallet_pubkey: Pubkey,
    game_id: u64,
) -> Result<CancelOutcome, String> {
    use crate::multiplayer::solana::tauri_signer::sign_and_send_via_tauri;
    use crate::solana::instructions::cancel_game_ix;

    let rpc = RpcClient::new_with_commitment(rpc_url.clone(), CommitmentConfig::confirmed());
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let data = match rpc.get_account_data(&game_pda) {
        Ok(d) => d,
        Err(e) => {
            let s = e.to_string();
            if s.contains("not found") || s.contains("AccountNotFound") {
                return Ok(CancelOutcome::NothingToRefund(
                    "game account does not exist".to_string(),
                ));
            }
            return Err(format!("fetch game account: {s}"));
        }
    };
    // Anchor discriminator (8) + game_id (8) precede white/black — see
    // `settlement_worker.rs::parse_game_account` for the same layout used
    // server-side.
    let white = data.get(16..48).map(Pubkey::try_from).and_then(Result::ok);
    let black = data.get(48..80).map(Pubkey::try_from).and_then(Result::ok);
    let (Some(white), Some(black)) = (white, black) else {
        return Ok(CancelOutcome::NothingToRefund(
            "game account malformed/empty".to_string(),
        ));
    };

    // Status byte follows white/black; wager_amount offset is pinned by the
    // program's `wager_amount_offset_is_212` test (+8 discriminator).
    // Statuses: 0=Pending 1=WaitingForOpponent 2=Active 3=Inactive 4=Disputed
    // 5=Finished 6=Settled 7=Expired 8=Cancelled (state/game.rs).
    const STATUS_OFFSET: usize = 8 + 8 + 32 + 32;
    const WAGER_OFFSET: usize = 8 + 212;
    let status = data.get(STATUS_OFFSET).copied().unwrap_or(0);
    let wager_amount = data
        .get(WAGER_OFFSET..WAGER_OFFSET + 8)
        .map(|b| u64::from_le_bytes(b.try_into().expect("8-byte slice")))
        .unwrap_or(0);

    if matches!(status, 5..=8) {
        return Ok(CancelOutcome::NothingToRefund(match status {
            5 => "game already finished".to_string(),
            6 => "game already settled".to_string(),
            7 => "game already expired".to_string(),
            _ => "game already cancelled".to_string(),
        }));
    }
    if status != 1 && status != 2 {
        // Pending / Inactive / Disputed — the on-chain cancel would reject
        // these states; don't burn a wallet popup on a doomed transaction.
        return Err(format!(
            "game {game_id} cannot be cancelled right now (status {status})"
        ));
    }

    let ix = cancel_game_ix(program_id, wallet_pubkey, white, black, game_id)
        .map_err(|e| format!("build cancel_game_ix: {e}"))?;

    let mut attempt = 0;
    loop {
        attempt += 1;
        match sign_and_send_via_tauri(
            &rpc_url,
            wallet_pubkey,
            &[ix.clone()],
            &[],
            "Cancelling wagered game",
        ) {
            Ok(sig) => {
                info!(
                    "[CANCEL_GAME] game {} cancelled on-chain, sig {}",
                    game_id, sig
                );
                return Ok(CancelOutcome::Refunded(sig));
            }
            Err(e) => {
                if attempt >= 3 || is_user_rejection(&e) {
                    return Err(e);
                }
                warn!(
                    "[CANCEL_GAME] attempt {} failed for game {}: {}, retrying...",
                    attempt, game_id, e
                );
                std::thread::sleep(std::time::Duration::from_millis(1000));
            }
        }
    }
}

pub fn spawn_cancel_lobby(
    rpc_url: String,
    program_id: Pubkey,
    wallet_pubkey: Option<Pubkey>,
    game_id: u64,
    wager_lamports: u64,
    node_id: Option<String>,
    tx: oneshot::Sender<Result<CancelOutcome, String>>,
) {
    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            let on_chain = if wager_lamports > 0 {
                let wallet = wallet_pubkey.ok_or_else(|| {
                    "Wallet unavailable; the wager could not be cancelled".to_string()
                });
                wallet.and_then(|wallet| cancel_game_on_chain(rpc_url, program_id, wallet, game_id))
            } else {
                Ok(CancelOutcome::NothingToRefund(
                    "free game; no escrow to refund".to_string(),
                ))
            };

            let relay_error = node_id.and_then(|node_id| {
                crate::multiplayer::vps_client::p2p_leave_game_fast(game_id.to_string(), &node_id)
                    .err()
            });
            if let Some(error) = relay_error {
                warn!(
                    "[LOBBY] Relay cleanup failed for cancelled game {}: {}",
                    game_id, error
                );
            }

            let result = match on_chain {
                Ok(outcome) => Ok(outcome),
                Err(error) => Err(error),
            };
            let _ = tx.send(result);
        })
        .detach();
}

async fn async_lookup_game(
    rpc_url: String,
    program_id: solana_sdk::pubkey::Pubkey,
    game_id: u64,
) -> Result<(u64, u64), String> {
    let game_pda = solana_sdk::pubkey::Pubkey::find_program_address(
        &[GAME_SEED, &game_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let rpc = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());
    let data = rpc
        .get_account_data(&game_pda)
        .map_err(|e| format!("get_account: {}", e))?;

    // Anchor account layout: 8-byte discriminator, then Borsh fields.
    // Game struct field order (see programs/xfchess-game/src/state/game.rs):
    //   game_id: u64 (8)  white: Pubkey (32)  black: Pubkey (32)  status: u8 (1)
    // disc(8) + game_id(8) + white(32) + black(32) = 80; status byte follows.
    const STATUS_OFFSET: usize = 8 + 8 + 32 + 32;
    if data.len() < STATUS_OFFSET + 1 {
        return Err("Account data too short for status".to_string());
    }
    let status_byte = data[STATUS_OFFSET];
    // GameStatus (Borsh discriminant order — see state/game.rs):
    // 0=Pending 1=WaitingForOpponent 2=Active 3=Inactive 4=Disputed
    // 5=Finished 6=Settled 7=Expired 8=Cancelled
    if status_byte != 1 {
        let label = match status_byte {
            0 => "pending",
            2 => "already full (Active)",
            3 => "Inactive",
            4 => "Disputed",
            5 => "Finished",
            6 => "Settled",
            7 => "Expired",
            8 => "Cancelled",
            _ => "unknown status",
        };
        return Err(format!(
            "Game {} is not available to join: {}",
            game_id, label
        ));
    }

    // wager_amount offset is pinned by a test in programs/xfchess-game/src/state/game.rs
    // (wager_amount_offset_is_212) — that test's value + 8 (discriminator) must match this.
    const WAGER_OFFSET: usize = 8 + 212;
    if data.len() < WAGER_OFFSET + 8 {
        return Err("Account data too short to read wager_amount".to_string());
    }
    let wager_lamports = u64::from_le_bytes(
        data[WAGER_OFFSET..WAGER_OFFSET + 8]
            .try_into()
            .map_err(|_| "slice error")?,
    );
    Ok((wager_lamports, game_id))
}

async fn async_join_game(
    rpc_url: String,
    wallet_pubkey: Pubkey,
    program_id: solana_sdk::pubkey::Pubkey,
    game_id: u64,
    global_session_keypair_bytes: Option<Vec<u8>>,
) -> Result<u64, String> {
    use crate::multiplayer::solana::tauri_signer::sign_via_tauri_only;
    use crate::multiplayer::vps_client;

    // Gate: joining any on-chain game requires the wager eligibility checks.
    crate::multiplayer::network::vps::identity::require_wager_eligibility(
        &wallet_pubkey.to_string(),
    )?;

    if let Some(kp_bytes) = global_session_keypair_bytes {
        return async_join_game_via_global_session(
            rpc_url,
            wallet_pubkey,
            program_id,
            game_id,
            kp_bytes,
        )
        .await;
    }

    // 1. Ask VPS for a session keypair for this game.
    // The VPS uses get-or-create semantics, so the same session pubkey that was
    // stored in game.fee_payer during create_game is returned here.
    let (session_pubkey_str, _) = vps_client::create_session(game_id, &wallet_pubkey.to_string())?;
    let session_pubkey: Pubkey = session_pubkey_str
        .parse()
        .map_err(|e| format!("parse session_pubkey: {e}"))?;

    // 2. Read the game account to get the white player pubkey for white_profile PDA.
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let rpc = solana_client::rpc_client::RpcClient::new_with_commitment(
        rpc_url.clone(),
        CommitmentConfig::confirmed(),
    );
    let game_data = rpc
        .get_account_data(&game_pda)
        .map_err(|e| format!("fetch game account: {e}"))?;
    // Game layout: 8 disc + 8 game_id + 32 white pubkey
    const WHITE_OFFSET: usize = 8 + 8;
    if game_data.len() < WHITE_OFFSET + 32 {
        return Err("game account too small to read white pubkey".to_string());
    }
    let white_bytes: [u8; 32] = game_data[WHITE_OFFSET..WHITE_OFFSET + 32]
        .try_into()
        .map_err(|_| "bad white bytes".to_string())?;
    let white_player = Pubkey::from(white_bytes);

    let join_ix = join_game_ix(
        program_id,
        wallet_pubkey,
        white_player,
        session_pubkey,
        game_id,
    )
    .map_err(|e| format!("build join_game_ix: {e}"))?;
    let auth_ix =
        authorize_session_key_ix(program_id, wallet_pubkey, game_id, session_pubkey, 86400)
            .map_err(|e| format!("build authorize_session_key_ix: {e}"))?;

    let ixs = vec![join_ix, auth_ix];

    // ONE wallet popup — signs everything together.
    let signed_bytes = sign_via_tauri_only(&rpc_url, wallet_pubkey, &ixs, &[], "Joining game")
        .map_err(|e| format!("sign bundled TX: {e}"))?;

    // VPS adds its session key co-signature and submits.
    vps_client::activate_session(game_id, &signed_bytes)?;

    Ok(game_id)
}

async fn async_join_game_via_global_session(
    rpc_url: String,
    wallet_pubkey: Pubkey,
    program_id: Pubkey,
    game_id: u64,
    session_keypair_bytes: Vec<u8>,
) -> Result<u64, String> {
    use crate::multiplayer::solana::global_session_manager::{
        build_global_join_game_ix, check_global_session_can_afford_wager, find_global_session_pda,
    };
    use crate::solana::instructions::{PROFILE_SEED, WAGER_ESCROW_SEED};
    use solana_sdk::signature::{Keypair, Signer};

    let session_kp = Keypair::try_from(session_keypair_bytes.as_slice())
        .map_err(|e| format!("session keypair: {e}"))?;
    let (session_pda, _bump) = find_global_session_pda(&program_id, &wallet_pubkey);
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;
    let player_profile_pda =
        Pubkey::find_program_address(&[PROFILE_SEED, wallet_pubkey.as_ref()], &program_id).0;

    let rpc = RpcClient::new_with_commitment(rpc_url.clone(), CommitmentConfig::confirmed());

    // Need the white player's pubkey (via the game account) to derive their
    // profile PDA, and a blockhash to build the tx. Neither read depends on
    // the other, so fetch them concurrently on separate threads rather than
    // blocking the executor on two sequential RPC round trips.
    let game_data_result = std::thread::scope(|scope| {
        let blockhash_handle =
            scope.spawn(|| rpc.get_latest_blockhash().map_err(|e| e.to_string()));
        let game_data = rpc
            .get_account_data(&game_pda)
            .map_err(|e| format!("fetch game account: {e}"));
        let blockhash = blockhash_handle
            .join()
            .map_err(|_| "blockhash fetch thread panicked".to_string())?
            .map_err(|e| format!("get_latest_blockhash: {e}"))?;
        game_data.map(|data| (data, blockhash))
    });
    let (game_data, blockhash) = game_data_result?;

    const WHITE_OFFSET: usize = 8 + 8;
    if game_data.len() < WHITE_OFFSET + 32 {
        return Err("game account too small to read white pubkey".to_string());
    }
    let white_bytes: [u8; 32] = game_data[WHITE_OFFSET..WHITE_OFFSET + 32]
        .try_into()
        .map_err(|_| "bad white bytes".to_string())?;
    let white_player = Pubkey::from(white_bytes);
    let white_profile_pda =
        Pubkey::find_program_address(&[PROFILE_SEED, white_player.as_ref()], &program_id).0;

    // Same pinned offset as `WAGER_OFFSET` elsewhere in this file
    // (`wager_amount_offset_is_212` on-chain test) — the game account we
    // just fetched already tells us the wager this join is committing to, no
    // extra RPC round trip needed for the affordability check below.
    const WAGER_AMOUNT_OFFSET: usize = 8 + 212;
    let wager_lamports = if game_data.len() >= WAGER_AMOUNT_OFFSET + 8 {
        u64::from_le_bytes(
            game_data[WAGER_AMOUNT_OFFSET..WAGER_AMOUNT_OFFSET + 8]
                .try_into()
                .unwrap_or_default(),
        )
    } else {
        0
    };
    check_global_session_can_afford_wager(&rpc_url, &session_pda, wager_lamports)?;

    let ix = build_global_join_game_ix(
        &program_id,
        &session_pda,
        &session_kp.pubkey(),
        &wallet_pubkey,
        &game_pda,
        &player_profile_pda,
        &white_profile_pda,
        &escrow_pda,
        game_id,
    );

    let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
        &[ix],
        Some(&session_kp.pubkey()),
        &[&session_kp],
        blockhash,
    );
    crate::multiplayer::solana::submit::submit_and_poll(
        &rpc,
        &tx,
        crate::multiplayer::solana::submit::SubmitConfig::fast(),
    )
    .map_err(|e| format!("global_join_game submit: {e}"))?;

    info!(
        "[JOIN_GAME] global_join_game landed for game {} (session-signed, no wallet popup)",
        game_id
    );

    // Best-effort: lets settlement_worker discover this game — see
    // `track_global_session_game`'s doc comment.
    if let Err(e) = crate::multiplayer::vps_client::track_global_session_game(
        game_id,
        &wallet_pubkey.to_string(),
    ) {
        warn!("[JOIN_GAME] track_global_session_game failed for {game_id}: {e}");
    }

    Ok(game_id)
}

// ---------------------------------------------------------------------------
// Bevy polling system
// ---------------------------------------------------------------------------

fn poll_lobby_tasks(
    mut lobby: ResMut<SolanaLobbyState>,
    mut sync: ResMut<crate::multiplayer::solana::addon::SolanaGameSync>,
    mut competitive: ResMut<crate::multiplayer::solana::addon::CompetitiveMatchState>,
    mut rollup_manager: ResMut<crate::multiplayer::rollup::manager::EphemeralRollupManager>,
    mut p2p_vps: ResMut<crate::multiplayer::network::p2p_vps::P2PVpsState>,
) {
    // Cancellation owns both the on-chain refund and relay removal. Do not
    // transition back to Idle until the result is known to the user.
    if let Some(ref mut rx) = lobby.cancel_rx {
        match rx.try_recv() {
            Ok(Ok(outcome)) => {
                let game_id = match lobby.status {
                    LobbyStatus::Cancelling { game_id } => game_id,
                    _ => 0,
                };
                let (refunded, message) = match outcome {
                    CancelOutcome::Refunded(sig) => {
                        crate::multiplayer::solana::wager_recovery::forget(game_id);
                        (true, format!("Wager refunded. Transaction: {sig}"))
                    }
                    CancelOutcome::NothingToRefund(message) => {
                        crate::multiplayer::solana::wager_recovery::forget(game_id);
                        (false, message)
                    }
                };
                lobby.status = LobbyStatus::Cancelled {
                    game_id,
                    refunded,
                    message,
                };
                lobby.cancel_rx = None;
                lobby.opponent_poll_rx = None;
                lobby.game_start_poll_rx = None;
                let game_id = game_id.to_string();
                if p2p_vps.hosting_game_id.as_deref() == Some(game_id.as_str()) {
                    p2p_vps.hosting_game_id = None;
                }
                if p2p_vps.joining_game_id.as_deref() == Some(game_id.as_str()) {
                    p2p_vps.joining_game_id = None;
                }
                sync.game_id = None;
                competitive.game_id = None;
                competitive.active = false;
                rollup_manager.game_id = 0;
            }
            Ok(Err(error)) => {
                let game_id = match lobby.status {
                    LobbyStatus::Cancelling { game_id } => game_id,
                    _ => 0,
                };
                lobby.status = LobbyStatus::CancelFailed { game_id, error };
                lobby.cancel_rx = None;
            }
            Err(oneshot::error::TryRecvError::Empty) => {}
            Err(_) => {
                let game_id = match lobby.status {
                    LobbyStatus::Cancelling { game_id } => game_id,
                    _ => 0,
                };
                lobby.status = LobbyStatus::CancelFailed {
                    game_id,
                    error: "Cancellation task stopped unexpectedly".to_string(),
                };
                lobby.cancel_rx = None;
            }
        }
    }

    // Poll transaction receiver.
    if let Some(ref mut rx) = lobby.tx_rx {
        match rx.try_recv() {
            Ok(Ok(game_id)) => {
                sync.game_id = Some(game_id);
                sync.wager_amount = lobby.wager_lamports();
                // Only wagered games go through the ER delegation flow that
                // move input waits on; stake-0 free games must stay playable
                // immediately (see `SolanaGameSync::requires_delegation`).
                sync.requires_delegation = lobby.wager_lamports() > 0;
                competitive.wager_lamports = lobby.wager_lamports();
                competitive.stake_amount = lobby.wager_lamports();
                competitive.game_id = Some(game_id);
                competitive.active = true;
                lobby.status = LobbyStatus::Success(game_id);
                lobby.tx_rx = None;
                crate::multiplayer::network::game_id_store::set(game_id);
                rollup_manager.game_id = game_id;
                rollup_manager.is_creator = lobby.mode == LobbyMode::Create;
                rollup_manager.used_global_session = lobby.last_attempt_used_global_session;
                info!(
                    "[LOBBY] Active game_id {} stored globally (rollup updated, is_creator={}, used_global_session={})",
                    game_id, rollup_manager.is_creator, rollup_manager.used_global_session
                );

                if lobby.mode == LobbyMode::Create {
                    // Every on-chain create (free or wagered) is announced to the
                    // same P2P relay used by plain online multiplayer, so a joiner
                    // finds it the same way regardless of stake. Free games (stake
                    // 0) are tagged "P2P" so they surface in the normal browse
                    // list rather than the Solana Browse tab's wagered-only filter.
                    let is_wagered = lobby.wager_sol > 0.0;
                    let game_type = if is_wagered { "solana_wager" } else { "P2P" };
                    let display_name = lobby
                        .cached_display_name
                        .clone()
                        .unwrap_or_else(|| "Anonymous".to_string());
                    let host_node_id = lobby
                        .cached_node_id
                        .clone()
                        .unwrap_or_else(|| "unknown_node_id".to_string());

                    let announce_result = if let Some(ref pwd) = lobby.room_password.clone() {
                        crate::multiplayer::vps_client::p2p_announce_game_with_password(
                            game_id.to_string(),
                            &host_node_id,
                            &display_name,
                            lobby.wager_sol as f64,
                            game_type,
                            lobby.time_control_base,
                            lobby.time_control_inc as u16,
                            Some(display_name.clone()),
                            if lobby.cached_elo > 0 {
                                Some(lobby.cached_elo)
                            } else {
                                None
                            },
                            lobby.cached_region.clone(),
                            pwd.clone(),
                        )
                    } else {
                        crate::multiplayer::vps_client::p2p_announce_game(
                            game_id.to_string(),
                            &host_node_id,
                            &display_name,
                            lobby.wager_sol as f64,
                            game_type,
                            lobby.time_control_base,
                            lobby.time_control_inc as u16,
                            Some(display_name.clone()),
                            if lobby.cached_elo > 0 {
                                Some(lobby.cached_elo)
                            } else {
                                None
                            },
                            lobby.cached_region.clone(),
                        )
                    };
                    if let Err(e) = announce_result {
                        warn!("[LOBBY] Failed to announce game {} to VPS: {}", game_id, e);
                        lobby.announce_warning = Some(format!(
                            "Couldn't list this game publicly ({e}) — share the Game ID directly instead."
                        ));
                    } else {
                        info!(
                            "[LOBBY] Announced game {} ({}) to VPS relay",
                            game_id, game_type
                        );

                        // Register as a discoverable host on the same P2P relay
                        // channel plain PvP uses (see `network::p2p_vps`). Without
                        // this, `poll_for_joiner_messages` never watches for the
                        // joiner's JOIN_ACK, so the host never learns the joiner's
                        // P2P node id and no transport (Iroh gossip or the relay
                        // fallback) ever gets wired up — the on-chain `join_game`
                        // can succeed while the two clients stay unconnected.
                        p2p_vps.hosting_game_id = Some(game_id.to_string());
                        p2p_vps.hosting_node_id = Some(host_node_id.clone());
                        p2p_vps.hosting_stake_amount = lobby.wager_sol as f64;
                        p2p_vps.hosting_base_secs = lobby.time_control_base;
                        p2p_vps.hosting_inc = lobby.time_control_inc as u16;
                        p2p_vps.host_poll_last = None; // poll immediately
                    }
                }
            }
            Ok(Err(e)) => {
                lobby.status = LobbyStatus::Error(e);
                lobby.tx_rx = None;
            }
            Err(oneshot::error::TryRecvError::Empty) => {}
            Err(_) => {
                lobby.status = LobbyStatus::Error("Task dropped".to_string());
                lobby.tx_rx = None;
            }
        }
    }

    // Poll opponent-joined receiver.
    if let Some(ref mut rx) = lobby.opponent_poll_rx {
        match rx.try_recv() {
            Ok(Ok(())) => {
                let game_id = match lobby.status {
                    LobbyStatus::WaitingForOpponent { game_id } => game_id,
                    _ => 0,
                };
                lobby.status = LobbyStatus::OpponentJoined { game_id };
                lobby.opponent_poll_rx = None;
            }
            Ok(Err(e)) => {
                lobby.status = LobbyStatus::Error(e);
                lobby.opponent_poll_rx = None;
            }
            Err(oneshot::error::TryRecvError::Empty) => {}
            Err(_) => {
                lobby.status = LobbyStatus::Error("Poll task dropped".to_string());
                lobby.opponent_poll_rx = None;
            }
        }
    }

    // Poll for the host's GAME_START signal (joiner side) — see
    // `WaitingForHostStart` in `screens.rs`'s auto-transition.
    if let Some(ref mut rx) = lobby.game_start_poll_rx {
        match rx.try_recv() {
            Ok(Ok(())) => {
                let game_id = match lobby.status {
                    LobbyStatus::WaitingForHostStart { game_id } => game_id,
                    _ => 0,
                };
                lobby.status = LobbyStatus::EnterGame { game_id };
                lobby.game_start_poll_rx = None;
            }
            Ok(Err(e)) => {
                lobby.status = LobbyStatus::Error(e);
                lobby.game_start_poll_rx = None;
            }
            Err(oneshot::error::TryRecvError::Empty) => {}
            Err(_) => {
                lobby.status = LobbyStatus::Error("Game-start poll task dropped".to_string());
                lobby.game_start_poll_rx = None;
            }
        }
    }

    // Poll lookup receiver.
    if let Some(ref mut rx) = lobby.lookup_rx {
        match rx.try_recv() {
            Ok(Ok((wager_lamports, game_id))) => {
                let wager_sol = wager_lamports as f64 / 1_000_000_000.0;
                lobby.status = LobbyStatus::Fetched { wager_sol, game_id };
                lobby.lookup_rx = None;
            }
            Ok(Err(e)) => {
                lobby.status = LobbyStatus::Error(e);
                lobby.lookup_rx = None;
            }
            Err(oneshot::error::TryRecvError::Empty) => {}
            Err(_) => {
                lobby.status = LobbyStatus::Error("Lookup dropped".to_string());
                lobby.lookup_rx = None;
            }
        }
    }
}

fn sync_from_solana_state(
    solana: Res<crate::multiplayer::solana::integration::SolanaIntegrationState>,
    mut lobby: ResMut<SolanaLobbyState>,
    region: Res<crate::multiplayer::social::BackendRegion>,
) {
    lobby.cached_balance = solana.balance;
    lobby.cached_rpc_url = DEVNET_RPC_URL.to_string();
    lobby.cached_elo = solana.cached_elo;
    // Re-synced every frame (not once-and-cached like the wallet pubkey
    // below) since authorization completes asynchronously in the background
    // and needs to flip this from None to Some without a reconnect.
    //
    // Forced to always `None` for now — global-session usage is disabled
    // (see `authorize_global_session_if_needed`'s doc comment) but
    // `try_load_global_session` at wallet-connect can still flip
    // `global_session_active` true from an already-valid local file/on-chain
    // session without going through that gate, so this is the actual
    // enforcement point every game-creation call site reads from.
    let _ = &solana.global_session_active;
    let _ = &solana.global_session_keypair;
    lobby.cached_global_session_keypair_bytes = None;
    if !region.tag.is_empty() {
        lobby.cached_region = Some(region.tag.clone());
    }
    if lobby.cached_display_name.is_none() {
        lobby.cached_display_name = solana.cached_display_name.clone();
    }

    if lobby.cached_keypair_bytes.is_none() {
        if let Some(ref pubkey) = solana.wallet_pubkey {
            lobby.cached_keypair_bytes = Some(pubkey.to_bytes().to_vec());

            // Kick off a one-time on-chain active-game check for the rejoin flow.
            if lobby.rejoin_rx.is_none() && lobby.rejoin_game_id.is_none() {
                let (tx, rx) = oneshot::channel();
                spawn_check_active_game(*pubkey, tx);
                lobby.rejoin_rx = Some(rx);
            }
        }
    }
}

pub fn spawn_check_active_game(wallet_pubkey: Pubkey, tx: oneshot::Sender<Option<u64>>) {
    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            // Enumerate up to 20 recent game IDs and check for an Active game owned by wallet.
            // In practice the backend /games/active/{wallet} endpoint would be faster.
            let result = crate::multiplayer::vps_client::get_active_game_for_wallet(
                &wallet_pubkey.to_string(),
            )
            .ok()
            .flatten();
            let _ = tx.send(result);
        })
        .detach();
}

fn poll_rejoin_check(mut lobby: ResMut<SolanaLobbyState>) {
    if let Some(ref mut rx) = lobby.rejoin_rx {
        match rx.try_recv() {
            Ok(maybe_id) => {
                lobby.rejoin_game_id = maybe_id;
                lobby.rejoin_rx = None;
            }
            Err(oneshot::error::TryRecvError::Empty) => {}
            Err(_) => {
                lobby.rejoin_rx = None;
            }
        }
    }
}

pub fn poll_tournament_games(mut lobby: ResMut<SolanaLobbyState>) {
    // Drain result if pending.
    if let Some(ref rx) = lobby.tournament_rx {
        if let Ok(games) = rx.try_recv() {
            lobby.tournament_games = games;
            lobby.tournament_rx = None;
        }
    }

    if lobby.mode != LobbyMode::Tournament {
        return;
    }
    let should_refresh = lobby
        .tournament_last_fetch
        .map(|t| t.elapsed().as_secs() >= 10)
        .unwrap_or(true);
    if !should_refresh || lobby.tournament_rx.is_some() {
        return;
    }
    lobby.tournament_last_fetch = Some(std::time::Instant::now());

    let (tx, rx) = crossbeam_channel::bounded(1);
    lobby.tournament_rx = Some(rx);
    std::thread::spawn(
        move || match crate::multiplayer::network::vps::list_tournament_games() {
            Ok(games) => {
                let _ = tx.send(games);
            }
            Err(e) => warn!("[SOLANA_TOURNEY] Failed to fetch tournament games: {}", e),
        },
    );
}

pub fn poll_solana_browse(mut lobby: ResMut<SolanaLobbyState>) {
    // Drain result if pending.
    if let Some(ref rx) = lobby.browse_rx {
        if let Ok(games) = rx.try_recv() {
            lobby.browse_games = games;
            lobby.browse_rx = None;
        }
    }

    // Only refresh when Browse tab is active.
    if lobby.mode != LobbyMode::Browse {
        return;
    }
    let should_refresh = lobby
        .browse_last_fetch
        .map(|t| t.elapsed().as_secs() >= 10)
        .unwrap_or(true);
    if !should_refresh || lobby.browse_rx.is_some() {
        return;
    }
    lobby.browse_last_fetch = Some(std::time::Instant::now());

    let (tx, rx) = crossbeam_channel::bounded(1);
    lobby.browse_rx = Some(rx);
    std::thread::spawn(
        move || match crate::multiplayer::vps_client::p2p_list_games() {
            Ok(games) => {
                let filtered: Vec<_> = games
                    .into_iter()
                    .filter(|g| g.game_type == "solana_wager" || g.game_type == "P2P")
                    .map(|g| crate::multiplayer::network::p2p_vps::VpsGameListing {
                        game_id: g.game_id,
                        display_name: g.display_name,
                        stake_amount: g.stake_amount,
                        game_type: g.game_type,
                        status: g.status,
                        base_time_seconds: g.base_time_seconds,
                        increment_seconds: g.increment_seconds,
                        username: g.username,
                        elo: g.elo,
                        region: g.region,
                        capacity: g.capacity,
                        players_joined: g.players_joined,
                        ttl_seconds: g.ttl_seconds,
                        is_private: g.is_private,
                    })
                    .collect();
                let _ = tx.send(filtered);
            }
            Err(e) => warn!("[SOLANA_BROWSE] Failed to fetch games: {}", e),
        },
    );
}
