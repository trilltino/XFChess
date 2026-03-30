//! Solana Lobby State
//!
//! Resource and plugin for the in-menu wager lobby (create/join a game on-chain).

use bevy::prelude::*;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use tokio::sync::oneshot;

use crate::multiplayer::solana::integration::DEVNET_RPC_URL;
use crate::solana::instructions::{
    authorize_session_key_ix, create_game_ix, init_profile_ix, join_game_ix, GameType, GAME_SEED,
    PROGRAM_ID as SOLANA_PROGRAM_ID, WAGER_ESCROW_SEED,
};

/// Which tab the lobby is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LobbyMode {
    #[default]
    Create,
    Join,
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
    Fetched { wager_sol: f64, game_id: u64 },
    /// Creator is waiting for opponent to sign join_game on-chain.
    WaitingForOpponent { game_id: u64 },
    /// Opponent detected on-chain — host can now start P2P.
    OpponentJoined { game_id: u64 },
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
    /// SOL amount chosen by creator (default 0.05).
    pub wager_sol: f32,
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
}

impl Default for SolanaLobbyState {
    fn default() -> Self {
        Self {
            mode: LobbyMode::default(),
            wager_sol: 0.05,
            game_id_input: String::new(),
            status: LobbyStatus::default(),
            tx_rx: None,
            lookup_rx: None,
            opponent_poll_rx: None,
            cached_balance: 0.0,
            cached_keypair_bytes: None,
            cached_rpc_url: DEVNET_RPC_URL.to_string(),
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
            .add_systems(Update, (sync_from_solana_state, poll_lobby_tasks).chain());
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
    tx: oneshot::Sender<Result<u64, String>>,
) {
    let program_id: solana_sdk::pubkey::Pubkey =
        SOLANA_PROGRAM_ID.parse().unwrap_or_default();

    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            let result = async_create_game(rpc_url, wallet_pubkey, program_id, wager_lamports).await;
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
    let program_id: solana_sdk::pubkey::Pubkey =
        SOLANA_PROGRAM_ID.parse().unwrap_or_default();

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
    let program_id: solana_sdk::pubkey::Pubkey =
        SOLANA_PROGRAM_ID.parse().unwrap_or_default();

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
    let program_id: solana_sdk::pubkey::Pubkey =
        SOLANA_PROGRAM_ID.parse().unwrap_or_default();

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
) -> Result<u64, String> {
    use crate::multiplayer::solana::tauri_signer::sign_via_tauri_only;
    use crate::multiplayer::rollup::vps_client;
    use std::time::{Duration, Instant};

    let game_id: u64 = rand::random();

    // 1. Ask VPS to generate session keypair → get session_pubkey.
    let session_pubkey_str = vps_client::create_session(game_id, &wallet_pubkey.to_string())
        .map_err(|e| format!("vps create_session: {e}"))?;
    let session_pubkey: Pubkey = session_pubkey_str
        .parse()
        .map_err(|e| format!("parse session_pubkey: {e}"))?;

    // 2. Build instructions — include init_profile if not yet created (no extra popup).
    use crate::solana::instructions::PROFILE_SEED;
    let rpc_check = RpcClient::new(rpc_url.clone());
    let profile_pda = Pubkey::find_program_address(
        &[PROFILE_SEED, wallet_pubkey.as_ref()],
        &program_id,
    ).0;
    let needs_profile = rpc_check.get_account(&profile_pda).is_err();

    let create_ix = create_game_ix(program_id, wallet_pubkey, game_id, wager_lamports, GameType::PvP)
        .map_err(|e| format!("build create_game_ix: {e}"))?;
    let auth_ix = authorize_session_key_ix(program_id, wallet_pubkey, game_id, session_pubkey)
        .map_err(|e| format!("build authorize_session_key_ix: {e}"))?;

    let mut ixs: Vec<solana_sdk::instruction::Instruction> = Vec::new();
    if needs_profile {
        if let Ok(ix) = init_profile_ix(program_id, wallet_pubkey) {
            ixs.push(ix);
        }
    }
    ixs.push(create_ix);
    ixs.push(auth_ix);

    // 3. ONE wallet popup — signs everything together.
    let signed_bytes = sign_via_tauri_only(&rpc_url, wallet_pubkey, &ixs, &[])
        .map_err(|e| format!("sign bundled TX: {e}"))?;

    // 4. VPS submits TX + funds session key (no more separate popups).
    vps_client::activate_session(game_id, &signed_bytes)
        .map_err(|e| format!("vps activate_session: {e}"))?;

    // Poll for game account to exist on-chain (max 60 seconds)
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let rpc = RpcClient::new(rpc_url);
    
    let start = Instant::now();
    let timeout = Duration::from_secs(60);
    let poll_interval = Duration::from_secs(1);
    
    info!("[CREATE_GAME] Waiting for game account {} to be confirmed on-chain...", game_pda);
    
    loop {
        if start.elapsed() > timeout {
            return Err(format!("Game account {} not found after 60s - transaction may have failed", game_pda));
        }
        
        match rpc.get_account(&game_pda) {
            Ok(_) => {
                info!("[CREATE_GAME] Game account {} confirmed on-chain for game {}", game_pda, game_id);
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
    let game_pda =
        solana_sdk::pubkey::Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;

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
        return Err(format!("Game {} is not available to join: {}", game_id, label));
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
    use crate::multiplayer::rollup::vps_client;
    use crate::solana::instructions::PROFILE_SEED;

    let rpc = RpcClient::new(&rpc_url);

    // 1. Ask VPS for a session keypair for this game.
    let session_pubkey_str = vps_client::create_session(game_id, &wallet_pubkey.to_string())
        .map_err(|e| format!("vps create_session: {e}"))?;
    let session_pubkey: Pubkey = session_pubkey_str
        .parse()
        .map_err(|e| format!("parse session_pubkey: {e}"))?;

    // 2. Build instructions — include init_profile if not yet created (no extra popup).
    let profile_pda = Pubkey::find_program_address(
        &[PROFILE_SEED, wallet_pubkey.as_ref()],
        &program_id,
    ).0;
    let needs_profile = rpc.get_account(&profile_pda).is_err();

    let join_ix = join_game_ix(program_id, wallet_pubkey, game_id)
        .map_err(|e| format!("build join_game_ix: {e}"))?;
    let auth_ix = authorize_session_key_ix(program_id, wallet_pubkey, game_id, session_pubkey)
        .map_err(|e| format!("build authorize_session_key_ix: {e}"))?;

    let mut ixs: Vec<solana_sdk::instruction::Instruction> = Vec::new();
    if needs_profile {
        if let Ok(ix) = init_profile_ix(program_id, wallet_pubkey) {
            ixs.push(ix);
        }
    }
    ixs.push(join_ix);
    ixs.push(auth_ix);

    // 3. ONE wallet popup — signs everything together.
    let signed_bytes = sign_via_tauri_only(&rpc_url, wallet_pubkey, &ixs, &[])
        .map_err(|e| format!("sign bundled TX: {e}"))?;

    // 3. VPS submits + funds.
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
                info!("[LOBBY] Active game_id {} stored globally (rollup updated, is_creator={})", game_id, rollup_manager.is_creator);
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
) {
    lobby.cached_balance = solana.balance;
    lobby.cached_rpc_url = crate::multiplayer::solana::integration::DEVNET_RPC_URL.to_string();

    if lobby.cached_keypair_bytes.is_none() {
        if let Some(ref pubkey) = solana.wallet_pubkey {
            lobby.cached_keypair_bytes = Some(pubkey.to_bytes().to_vec());
        }
    }
}
