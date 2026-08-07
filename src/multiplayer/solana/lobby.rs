//! Solana Lobby State
//!
//! Resource and plugin for the in-menu wager lobby (create/join a game on-chain).

use bevy::prelude::*;
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{pubkey::Pubkey, signature::Signature};
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

use crate::multiplayer::solana::integration::state::DEVNET_RPC_URL;
use crate::solana::instructions::{
    accept_draw_ix, authorize_session_key_ix, create_game_ix, join_game_ix, offer_draw_ix,
    GAME_SEED, PROGRAM_ID as SOLANA_PROGRAM_ID,
};

/// Which tab the lobby is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LobbyMode {
    #[default]
    Create,
    Join,
    Browse,
    /// On-chain games created by the backend tournament orchestrator.
    Tournament,
}

/// ELO range matching preference for matchmaking.
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

/// Async task outcome communicated back to the Bevy system.
#[derive(Debug, Clone)]
pub enum LobbyStatus {
    Idle,
    /// Transaction or lookup in flight.
    Pending,
    /// Game was created or joined successfully — stores the game_id.
    Success(u64),
    /// RPC returned a wager amount for a join lookup.
    Fetched {
        wager_sol: f64,
        game_id: u64,
    },
    /// Creator is waiting for opponent to sign join_game on-chain.
    WaitingForOpponent {
        game_id: u64,
    },
    /// Opponent detected on-chain — host can now start P2P.
    OpponentJoined {
        game_id: u64,
    },
    /// We (the joiner) have confirmed our on-chain `join_game` — now waiting
    /// for the host to click "Host Game", which relays a GAME_START signal
    /// over the P2P relay (see `spawn_poll_game_start`).
    WaitingForHostStart {
        game_id: u64,
    },
    /// The host's GAME_START signal arrived — safe to actually enter the match.
    EnterGame {
        game_id: u64,
    },
    Error(String),
}

impl Default for LobbyStatus {
    fn default() -> Self {
        Self::Idle
    }
}

/// Central UI state for the Solana wager lobby.
#[derive(Resource)]
pub struct SolanaLobbyState {
    pub mode: LobbyMode,
    /// Whether the "Create Game" tab is shown. True when the lobby is entered
    /// via "Wagered PVP"; false via "Find Wagered Game" (join/browse only).
    pub allow_create: bool,
    /// SOL amount chosen by creator. Defaults to 0 — the Create Game panel
    /// should always open showing a $0 wager, not a leftover/sticky amount.
    pub wager_sol: f32,
    /// Raw text typed into the custom wager amount field (USD when a live
    /// rate is available, otherwise SOL). Kept separate from `wager_sol` so
    /// in-progress typing (e.g. "1.") isn't clobbered by reformatting.
    pub wager_amount_input: String,
    /// Match type: 0=Free Casual, 1=Free Rated (ELO), 2=Wagered.
    pub match_type: u8,
    /// Raw game-id text typed by the joiner.
    pub game_id_input: String,
    pub status: LobbyStatus,
    /// Channel receiving the result of a create/join transaction.
    pub tx_rx: Option<oneshot::Receiver<Result<u64, String>>>,
    /// Channel receiving the result of a game-info lookup (wager in lamports).
    pub lookup_rx: Option<oneshot::Receiver<Result<(u64, u64), String>>>,
    /// Channel receiving notification that opponent joined on-chain.
    pub opponent_poll_rx: Option<oneshot::Receiver<Result<(), String>>>,
    /// Channel receiving notification that the host marked the game
    /// in-progress on the relay (see `spawn_poll_game_start`).
    pub game_start_poll_rx: Option<oneshot::Receiver<Result<(), String>>>,
    // Cached from SolanaIntegrationState each frame.
    pub cached_balance: f64,
    pub cached_keypair_bytes: Option<Vec<u8>>,
    pub cached_rpc_url: String,
    /// Session keypair bytes when an `authorize_global_session` has already
    /// gone through (see `authorize_global_session_if_needed`) — passing
    /// this to `spawn_create_game`/`spawn_join_game` skips the wallet popup
    /// entirely. `None` while unauthorized, falling back to the original
    /// per-game wallet-signed flow.
    pub cached_global_session_keypair_bytes: Option<Vec<u8>>,
    /// Whether the in-flight (or just-completed) create/join attempt used
    /// the global session — set at the moment `spawn_create_game`/
    /// `spawn_join_game` is called, not re-derived later from
    /// `cached_global_session_keypair_bytes`, since that can flip between
    /// spawning the attempt and its result landing (e.g. authorization
    /// completing mid-flight). `poll_lobby_tasks` copies this onto
    /// `EphemeralRollupManager::used_global_session` once the attempt
    /// succeeds, which is what delegation actually keys off.
    pub last_attempt_used_global_session: bool,
    /// Cached display name used when announcing wagered games to the VPS relay.
    pub cached_display_name: Option<String>,
    /// Cached node ID used when announcing wagered games to the VPS relay.
    pub cached_node_id: Option<String>,
    /// Cached raw Iroh secret key, refreshed alongside `cached_node_id` —
    /// used to sign JOIN_ACK/relay messages so the backend can verify the
    /// sender actually controls the claimed `cached_node_id`, not just
    /// asserts it. See `p2p_relay/routes.rs::send_message`'s verification.
    pub cached_secret_key_bytes: Option<[u8; 32]>,
    /// Cached ELO from on-chain profile; 0 = unknown.
    pub cached_elo: u16,
    /// Cached VPS/backend region tag (e.g. "eu-central").
    pub cached_region: Option<String>,
    /// Optional room password (None = public).
    pub room_password: Option<String>,
    /// Time control: base seconds (default 300 = 5 min).
    pub time_control_base: u32,
    /// Time control: increment seconds per move (default 0).
    pub time_control_inc: u32,
    /// ELO matching preference.
    pub elo_pref: EloMatchPref,
    /// Receiver for the on-chain active-game check (rejoin flow).
    pub rejoin_rx: Option<oneshot::Receiver<Option<u64>>>,
    /// Game ID found during rejoin check (displayed until dismissed).
    pub rejoin_game_id: Option<u64>,
    /// Cached wagered game listings for the Browse tab.
    pub browse_games: Vec<crate::multiplayer::network::p2p_vps::VpsGameListing>,
    /// Last time the browse list was fetched.
    pub browse_last_fetch: Option<std::time::Instant>,
    /// Receiver for background browse-list fetch.
    pub browse_rx: Option<
        crossbeam_channel::Receiver<Vec<crate::multiplayer::network::p2p_vps::VpsGameListing>>,
    >,
    /// Cached backend-tournament games for the Tournament tab (only matches
    /// with an on-chain Solana game_id).
    pub tournament_games: Vec<crate::multiplayer::network::vps::TournamentGameListing>,
    /// Last time the tournament-games list was fetched.
    pub tournament_last_fetch: Option<std::time::Instant>,
    /// Receiver for background tournament-games fetch.
    pub tournament_rx: Option<
        crossbeam_channel::Receiver<Vec<crate::multiplayer::network::vps::TournamentGameListing>>,
    >,
    /// Set when the post-create P2P announce (which makes the game show up
    /// in a peer's Browse Games list) fails. The game itself is still fully
    /// playable via a direct Game ID share — this is surfaced so a host isn't
    /// left wondering why nobody can find their game with zero on-screen sign
    /// anything went wrong (previously only a server-console `warn!`).
    pub announce_warning: Option<String>,
    /// Last time this host sent a P2P-relay heartbeat while waiting for an
    /// opponent. Without this, the Browse Games listing created by the
    /// post-create announce silently falls out of the backend's stale-lobby
    /// sweep after `LOBBY_TTL_SECS` (90s) even though the on-chain game (and
    /// this "Waiting for opponent" screen) is still fully live — a host
    /// waiting longer than that becomes invisible to browsers with no
    /// on-screen indication anything changed.
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
    /// Wager in lamports (from the `wager_sol` field).
    pub fn wager_lamports(&self) -> u64 {
        (self.wager_sol as f64 * 1_000_000_000.0) as u64
    }
}

/// Plugin — registers the resource and polling system.
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

/// Spawn a `create_game` transaction on `IoTaskPool`.
/// `global_session_keypair_bytes`: when `Some` (an already-authorized global
/// session — see `integration::systems::authorize_global_session_if_needed`),
/// the game is created and signed entirely with that session key, no Tauri
/// wallet popup at all. When `None`, falls back to the original per-game
/// wallet-signed flow.
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
            let _ = tx.send(result);
        })
        .detach();
}

/// Fire-and-forget: record a draw offer on the game's on-chain account. The
/// off-chain P2P `DrawOfferEvent` already gives the opponent instant UI
/// feedback (see `crate::ui::game::game_ui`) — this just makes the on-chain
/// `Game` account agree once the opponent accepts, so settlement (which reads
/// on-chain state only) actually pays out.
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

/// Fire-and-forget: accept the opponent's pending on-chain draw offer, ending
/// the game as a draw. Settlement (pot split, ELO, stats) happens afterward
/// through the backend's existing generic settlement sweep, which finalizes
/// any game whose on-chain status is `Finished` — same path as checkmate,
/// resignation, or timeout, so no separate handling is needed here.
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

/// Spawn a game-info lookup on `IoTaskPool` (returns wager_lamports + game_id).
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

/// Spawn a background task that polls the P2P relay every 2 s for this
/// game's durable status to flip to `InProgress` (set by the host's "Host
/// Game" button via `p2p_accept_join`). Times out after 10 minutes.
pub fn spawn_poll_game_start(game_id: u64, tx: oneshot::Sender<Result<(), String>>) {
    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            let result = async_poll_game_start(game_id).await;
            let _ = tx.send(result);
        })
        .detach();
}

/// Spawn a background task that polls the on-chain game account every 3 s until
/// the `black` pubkey is set (opponent joined), then resolves the oneshot.
/// Times out after 5 minutes.
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

/// Spawn a `join_game` transaction on `IoTaskPool`.
/// See `spawn_create_game`'s doc comment — same `global_session_keypair_bytes`
/// contract (`Some` = zero-popup join via the global session, `None` = the
/// original per-game wallet-signed flow).
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
            let _ = tx.send(result);
        })
        .detach();
}

// ---------------------------------------------------------------------------
// Private async implementations
// ---------------------------------------------------------------------------

/// Polls every 3 s for up to 5 min until the game account's `black` field is
/// set to a non-default pubkey (meaning opponent has called `join_game`).
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

/// Polls the P2P relay's game listing every 2 s for up to 10 min until this
/// game's entry reports `status == "InProgress"`. Deliberately checks the
/// relay's durable per-game status (flipped once, server-side, by
/// `p2p_accept_join`) rather than a one-shot message: a message log can
/// accumulate a stale `GAME_START` from an earlier test/attempt against the
/// same `game_id` (nothing expires individual messages — see
/// `backend/src/signing/p2p_relay/state.rs`), which would make a *new* join
/// falsely believe the host had already started. A status flip has no such
/// history to misread.
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
    use std::time::{Duration, Instant};

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
    let (session_pubkey_str, platform_fee_lamports) =
        vps_client::create_session(game_id, &wallet_pubkey.to_string())?;
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
    let signed_bytes = sign_via_tauri_only(&rpc_url, wallet_pubkey, &ixs, &[], label)
        .map_err(|e| format!("sign bundled TX: {e}"))?;

    // 4. VPS submits TX + funds session key (no more separate popups).
    vps_client::activate_session(game_id, &signed_bytes)?;

    // Poll for game account to exist on-chain (max 60 seconds)
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    // `confirmed`, not the client default `finalized` — the VPS already waits for
    // confirmed status before returning, so polling at `finalized` here would add
    // another ~10-20s on top for no additional safety.
    let rpc = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());

    let start = Instant::now();
    let timeout = Duration::from_secs(60);
    let poll_interval = Duration::from_millis(150);

    info!(
        "[CREATE_GAME] Waiting for game account {} to be confirmed on-chain...",
        game_pda
    );

    loop {
        if start.elapsed() > timeout {
            return Err(format!(
                "Game account {} not found after 60s - transaction may have failed",
                game_pda
            ));
        }

        match rpc.get_account(&game_pda) {
            Ok(_) => {
                info!(
                    "[CREATE_GAME] Game account {} confirmed on-chain for game {}",
                    game_pda, game_id
                );
                break;
            }
            Err(_) => {
                std::thread::sleep(poll_interval);
            }
        }
    }

    Ok(game_id)
}

/// Zero-popup create: signs + submits `global_create_game` directly with an
/// already-authorized global session keypair. No Tauri round-trip, no VPS
/// round-trip for the transaction itself — the session key pays its own tx
/// fee (funded once, during authorization) and wager funds are drawn from
/// the on-chain `GlobalSessionDelegation` vault, not the wallet.
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
        build_global_create_game_ix, find_global_session_pda,
    };
    use crate::solana::instructions::WAGER_ESCROW_SEED;
    use solana_sdk::signature::{Keypair, Signer};
    use solana_sdk::transaction::Transaction;
    use std::time::Instant;

    let session_kp = Keypair::try_from(session_keypair_bytes.as_slice())
        .map_err(|e| format!("session keypair: {e}"))?;
    let (session_pda, _bump) = find_global_session_pda(&program_id, &wallet_pubkey);
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;

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
    let blockhash = rpc
        .get_latest_blockhash()
        .map_err(|e| format!("get_latest_blockhash: {e}"))?;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&session_kp.pubkey()),
        &[&session_kp],
        blockhash,
    );

    let start = Instant::now();
    fast_send_and_confirm(&rpc, &tx).map_err(|e| format!("global_create_game submit: {e}"))?;

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

/// Zero-popup join: signs + submits `global_join_game` directly with an
/// already-authorized global session keypair — same trade-offs as
/// `async_create_game_via_global_session`.
async fn async_join_game_via_global_session(
    rpc_url: String,
    wallet_pubkey: Pubkey,
    program_id: Pubkey,
    game_id: u64,
    session_keypair_bytes: Vec<u8>,
) -> Result<u64, String> {
    use crate::multiplayer::solana::global_session_manager::{
        build_global_join_game_ix, find_global_session_pda,
    };
    use crate::solana::instructions::{PROFILE_SEED, WAGER_ESCROW_SEED};
    use solana_sdk::signature::{Keypair, Signer};
    use solana_sdk::transaction::Transaction;

    let session_kp = Keypair::try_from(session_keypair_bytes.as_slice())
        .map_err(|e| format!("session keypair: {e}"))?;
    let (session_pda, _bump) = find_global_session_pda(&program_id, &wallet_pubkey);
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;
    let player_profile_pda =
        Pubkey::find_program_address(&[PROFILE_SEED, wallet_pubkey.as_ref()], &program_id).0;

    // Need the white player's pubkey to derive their profile PDA — read the
    // game account (confirmed commitment; the account only needs to exist,
    // not be freshly written).
    let rpc = RpcClient::new_with_commitment(rpc_url.clone(), CommitmentConfig::confirmed());
    let game_data = rpc
        .get_account_data(&game_pda)
        .map_err(|e| format!("fetch game account: {e}"))?;
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

    let blockhash = rpc
        .get_latest_blockhash()
        .map_err(|e| format!("get_latest_blockhash: {e}"))?;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&session_kp.pubkey()),
        &[&session_kp],
        blockhash,
    );
    fast_send_and_confirm(&rpc, &tx).map_err(|e| format!("global_join_game submit: {e}"))?;

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

/// Polls in-flight oneshot channels each frame; updates LobbyStatus on completion.
fn poll_lobby_tasks(
    mut lobby: ResMut<SolanaLobbyState>,
    mut sync: ResMut<crate::multiplayer::solana::addon::SolanaGameSync>,
    mut competitive: ResMut<crate::multiplayer::solana::addon::CompetitiveMatchState>,
    mut rollup_manager: ResMut<crate::multiplayer::rollup::manager::EphemeralRollupManager>,
    mut p2p_vps: ResMut<crate::multiplayer::network::p2p_vps::P2PVpsState>,
) {
    // Poll transaction receiver.
    if let Some(ref mut rx) = lobby.tx_rx {
        match rx.try_recv() {
            Ok(Ok(game_id)) => {
                sync.game_id = Some(game_id);
                sync.wager_amount = lobby.wager_lamports();
                competitive.wager_lamports = lobby.wager_lamports();
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

/// Copies balance and keypair bytes from `SolanaIntegrationState` into
/// `SolanaLobbyState` so the UI can read them without an extra SystemParam.
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
    lobby.cached_global_session_keypair_bytes = match solana.global_session_active {
        true => solana
            .global_session_keypair
            .as_ref()
            .map(|kp| kp.to_bytes().to_vec()),
        false => None,
    };
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

/// Spawn a task that looks for an active on-chain game belonging to `wallet`.
/// Resolves to Some(game_id) if one is found, None otherwise.
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

/// Poll the rejoin check receiver.
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

/// Poll background tournament-games fetch and auto-refresh every 10 seconds
/// while the Tournament tab is active. Lists only backend-tournament matches
/// that have an on-chain Solana game_id.
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

/// Poll background browse-list fetch and auto-refresh every 10 seconds.
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

fn fast_send_and_confirm(
    rpc: &RpcClient,
    tx: &solana_sdk::transaction::Transaction,
) -> Result<Signature, String> {
    use solana_client::rpc_config::RpcSendTransactionConfig;
    let config = RpcSendTransactionConfig {
        skip_preflight: true,
        ..Default::default()
    };
    let sig = rpc
        .send_transaction_with_config(tx, config)
        .map_err(|e| format!("send_transaction: {e}"))?;

    let commitment = CommitmentConfig::confirmed();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if Instant::now() > deadline {
            return Err(format!("confirmation timeout for signature {sig}"));
        }
        match rpc.get_signature_status_with_commitment(&sig, commitment) {
            Ok(Some(Ok(()))) => return Ok(sig),
            Ok(Some(Err(e))) => return Err(format!("transaction failed (sig={sig}): {e:?}")),
            Ok(None) | Err(_) => std::thread::sleep(Duration::from_millis(150)),
        }
    }
}
