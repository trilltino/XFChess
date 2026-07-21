//! Solana Lobby State
//!
//! Resource and plugin for the in-menu wager lobby (create/join a game on-chain).

use bevy::prelude::*;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use tokio::sync::oneshot;

use crate::multiplayer::solana::integration::state::DEVNET_RPC_URL;
use crate::solana::instructions::{
    authorize_session_key_ix, create_game_ix, join_game_ix, GAME_SEED,
    PROGRAM_ID as SOLANA_PROGRAM_ID,
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
    /// SOL amount chosen by creator (default 0.05).
    pub wager_sol: f32,
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
    // Cached from SolanaIntegrationState each frame.
    pub cached_balance: f64,
    pub cached_keypair_bytes: Option<Vec<u8>>,
    pub cached_rpc_url: String,
    /// Cached display name used when announcing wagered games to the VPS relay.
    pub cached_display_name: Option<String>,
    /// Cached node ID used when announcing wagered games to the VPS relay.
    pub cached_node_id: Option<String>,
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
    /// Session key expires_at Unix timestamp (from on-disk session; populated by sync_from_solana_state).
    pub session_expires_at: Option<i64>,
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
}

impl Default for SolanaLobbyState {
    fn default() -> Self {
        Self {
            mode: LobbyMode::default(),
            allow_create: true,
            wager_sol: 0.05,
            match_type: 0,
            game_id_input: String::new(),
            status: LobbyStatus::default(),
            tx_rx: None,
            lookup_rx: None,
            opponent_poll_rx: None,
            cached_balance: 0.0,
            cached_keypair_bytes: None,
            cached_rpc_url: DEVNET_RPC_URL.to_string(),
            cached_display_name: None,
            cached_node_id: None,
            cached_elo: 0,
            cached_region: None,
            room_password: None,
            time_control_base: 300,
            time_control_inc: 0,
            elo_pref: EloMatchPref::default(),
            session_expires_at: None,
            rejoin_rx: None,
            rejoin_game_id: None,
            browse_games: Vec::new(),
            browse_last_fetch: None,
            browse_rx: None,
            tournament_games: Vec::new(),
            tournament_last_fetch: None,
            tournament_rx: None,
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
pub fn spawn_create_game(
    rpc_url: String,
    wallet_pubkey: Pubkey,
    wager_lamports: u64,
    match_type: u8,
    time_base: u32,
    time_inc: u32,
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
            )
            .await;
            let _ = tx.send(result);
        })
        .detach();
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
pub fn spawn_join_game(
    rpc_url: String,
    wallet_pubkey: Pubkey,
    game_id: u64,
    tx: oneshot::Sender<Result<u64, String>>,
) {
    let program_id: solana_sdk::pubkey::Pubkey = SOLANA_PROGRAM_ID.parse().unwrap_or_default();

    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            let result = async_join_game(rpc_url, wallet_pubkey, program_id, game_id).await;
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

    let rpc = solana_client::rpc_client::RpcClient::new(rpc_url);
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

async fn async_create_game(
    rpc_url: String,
    wallet_pubkey: Pubkey,
    program_id: solana_sdk::pubkey::Pubkey,
    wager_lamports: u64,
    match_type: u8,
    time_base: u32,
    time_inc: u32,
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

    // 1. Ask VPS to generate session keypair → get session_pubkey + platform fee.
    let (session_pubkey_str, platform_fee_lamports) =
        vps_client::create_session(game_id, &wallet_pubkey.to_string())
            .map_err(|e| format!("vps create_session: {e}"))?;
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
    let signed_bytes = sign_via_tauri_only(&rpc_url, wallet_pubkey, &ixs, &[])
        .map_err(|e| format!("sign bundled TX: {e}"))?;

    // 4. VPS submits TX + funds session key (no more separate popups).
    vps_client::activate_session(game_id, &signed_bytes)
        .map_err(|e| format!("vps activate_session: {e}"))?;

    // Poll for game account to exist on-chain (max 60 seconds)
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let rpc = RpcClient::new(rpc_url);

    let start = Instant::now();
    let timeout = Duration::from_secs(60);
    let poll_interval = Duration::from_secs(1);

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

    let rpc = RpcClient::new(rpc_url);
    let data = rpc
        .get_account_data(&game_pda)
        .map_err(|e| format!("get_account: {}", e))?;

    // Anchor account layout: 8-byte discriminator, then Borsh fields.
    // Game struct field order (see programs/xfchess-game/src/state/game.rs):
    //   game_id: u64 (8)  white: Pubkey (32)  black: Pubkey (32)
    //   status: u8 (1)    result: u8 (1)
    //   fen: String (4 + len)  move_count: u16 (2)  turn: u8 (1)
    //   created_at: i64 (8)   updated_at: i64 (8)
    //   wager_amount: u64 (8)
    //
    // Minimum offset to wager_amount:
    //   disc(8) + game_id(8) + white(32) + black(32) + status(1) + result(1)
    //   + fen_len_prefix(4) + fen_bytes + move_count(2) + turn(1) + created_at(8) + updated_at(8)
    //
    // The FEN string length varies, so we parse it dynamically.
    // disc(8) + game_id(8) + white(32) + black(32) = 80; status byte follows.
    const STATUS_OFFSET: usize = 8 + 8 + 32 + 32;
    if data.len() < STATUS_OFFSET + 1 {
        return Err("Account data too short for status".to_string());
    }
    let status_byte = data[STATUS_OFFSET];
    // GameStatus: 0=WaitingForOpponent, 1=Active, 2=Finished, 3=Expired
    if status_byte != 0 {
        let label = match status_byte {
            1 => "already full (Active)",
            2 => "Finished",
            3 => "Expired",
            _ => "unknown status",
        };
        return Err(format!(
            "Game {} is not available to join: {}",
            game_id, label
        ));
    }

    let offset = parse_wager_offset(&data)?;
    if data.len() < offset + 8 {
        return Err("Account data too short to read wager_amount".to_string());
    }
    let wager_lamports = u64::from_le_bytes(
        data[offset..offset + 8]
            .try_into()
            .map_err(|_| "slice error")?,
    );
    Ok((wager_lamports, game_id))
}

/// Walk the Borsh-encoded Game account to find the wager_amount offset.
fn parse_wager_offset(data: &[u8]) -> Result<usize, String> {
    const FIXED_HEADER: usize = 8 + 8 + 32 + 32 + 1 + 1; // disc + game_id + white + black + status + result
    if data.len() < FIXED_HEADER + 4 {
        return Err("Data too short for FEN prefix".to_string());
    }
    let fen_len = u32::from_le_bytes(
        data[FIXED_HEADER..FIXED_HEADER + 4]
            .try_into()
            .map_err(|_| "fen_len slice err")?,
    ) as usize;
    // After fen string: move_count (u16) + turn (u8) + created_at (i64) + updated_at (i64)
    let after_fen = FIXED_HEADER + 4 + fen_len;
    Ok(after_fen + 2 + 1 + 8 + 8)
}

async fn async_join_game(
    rpc_url: String,
    wallet_pubkey: Pubkey,
    program_id: solana_sdk::pubkey::Pubkey,
    game_id: u64,
) -> Result<u64, String> {
    use crate::multiplayer::solana::tauri_signer::sign_via_tauri_only;
    use crate::multiplayer::vps_client;

    // Gate: joining any on-chain game requires the wager eligibility checks.
    crate::multiplayer::network::vps::identity::require_wager_eligibility(
        &wallet_pubkey.to_string(),
    )?;

    // 1. Ask VPS for a session keypair for this game.
    // The VPS uses get-or-create semantics, so the same session pubkey that was
    // stored in game.fee_payer during create_game is returned here.
    let (session_pubkey_str, _) = vps_client::create_session(game_id, &wallet_pubkey.to_string())
        .map_err(|e| format!("vps create_session: {e}"))?;
    let session_pubkey: Pubkey = session_pubkey_str
        .parse()
        .map_err(|e| format!("parse session_pubkey: {e}"))?;

    // 2. Read the game account to get the white player pubkey for white_profile PDA.
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let rpc = solana_client::rpc_client::RpcClient::new(rpc_url.clone());
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
    let signed_bytes = sign_via_tauri_only(&rpc_url, wallet_pubkey, &ixs, &[])
        .map_err(|e| format!("sign bundled TX: {e}"))?;

    // VPS adds its session key co-signature and submits.
    vps_client::activate_session(game_id, &signed_bytes)
        .map_err(|e| format!("vps activate_session: {e}"))?;

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
                info!(
                    "[LOBBY] Active game_id {} stored globally (rollup updated, is_creator={})",
                    game_id, rollup_manager.is_creator
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
                    } else {
                        info!(
                            "[LOBBY] Announced game {} ({}) to VPS relay",
                            game_id, game_type
                        );
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
    if !region.tag.is_empty() {
        lobby.cached_region = Some(region.tag.clone());
    }
    if lobby.cached_display_name.is_none() {
        lobby.cached_display_name = solana.cached_display_name.clone();
    }

    if lobby.cached_keypair_bytes.is_none() {
        if let Some(ref pubkey) = solana.wallet_pubkey {
            lobby.cached_keypair_bytes = Some(pubkey.to_bytes().to_vec());

            // Read session key expiry once per wallet connection.
            if lobby.session_expires_at.is_none() {
                lobby.session_expires_at =
                    crate::multiplayer::solana::session_key_manager::SessionKeyManager::expires_at(
                        pubkey,
                    );
            }

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
