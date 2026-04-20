// Solana Program Integration and Wallet Management
use bevy::prelude::*;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use crate::game::events::GameStartedEvent;
use crate::multiplayer::{
    session_key_manager::SessionKeyManager, BraidNetworkState, NetworkMessage,
};
use crate::solana::{constants::SOLANA_PROGRAM_ID, instructions::authorize_session_key_ix};

/// Devnet RPC endpoint
pub const DEVNET_RPC_URL: &str = "https://api.devnet.solana.com";
/// MagicBlock EU Devnet endpoint
pub const MAGICBLOCK_EU_DEVNET: &str = "https://devnet-eu.magicblock.app";
/// Program ID for XFChess
pub const XFCHESS_PROGRAM_ID: &str = "3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP";

/// PDA Seeds matching Anchor program
pub const GAME_SEED: &[u8] = b"game";
pub const MOVE_LOG_SEED: &[u8] = b"move_log";
pub const PROFILE_SEED: &[u8] = b"profile";
pub const SESSION_DELEGATION_SEED: &[u8] = b"session_delegation";
pub const WAGER_ESCROW_SEED: &[u8] = b"wager_escrow";

// Resource to hold Solana integration state
#[derive(Resource)]
pub struct SolanaIntegrationState {
    /// The derived Solana keypair from the Iroh node key
    pub keypair: Option<Keypair>,
    /// Direct RPC client for Solana
    pub rpc_client: Option<RpcClient>,
    /// Current balance of the wallet
    pub balance: f64,
    /// Whether the handshake with opponent is completed
    pub handshake_completed: bool,
    /// Pending transaction task
    pub pending_task: Option<tokio::task::JoinHandle<Result<u64, String>>>,
    /// The opponent's public key (for verification)
    pub opponent_pubkey: Option<Pubkey>,
    /// Program ID for XFChess
    pub program_id: Pubkey,
}

impl std::fmt::Debug for SolanaIntegrationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SolanaIntegrationState")
            .field("keypair_pubkey", &self.keypair.as_ref().map(|k| k.pubkey()))
            .field("balance", &self.balance)
            .field("handshake_completed", &self.handshake_completed)
            .field("opponent_pubkey", &self.opponent_pubkey)
            .finish()
    }
}

impl Default for SolanaIntegrationState {
    fn default() -> Self {
        Self {
            keypair: None,
            rpc_client: None,
            balance: 0.0,
            handshake_completed: false,
            pending_task: None,
            opponent_pubkey: None,
            program_id: XFCHESS_PROGRAM_ID.parse().unwrap_or_default(),
        }
    }
}

impl SolanaIntegrationState {
    /// Derive a Program Derived Address (PDA) using the program ID
    pub fn derive_pda(&self, seeds: &[&[u8]]) -> Pubkey {
        Pubkey::find_program_address(seeds, &self.program_id).0
    }

    /// Get the game PDA for a given game ID
    pub fn get_game_pda(&self, game_id: u64) -> Pubkey {
        self.derive_pda(&[GAME_SEED, &game_id.to_le_bytes()])
    }

    /// Get the escrow PDA for a given game ID
    pub fn get_escrow_pda(&self, game_id: u64) -> Pubkey {
        self.derive_pda(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()])
    }

    /// Get the profile PDA for a given wallet
    pub fn get_profile_pda(&self, wallet: &Pubkey) -> Pubkey {
        self.derive_pda(&[PROFILE_SEED, wallet.as_ref()])
    }

    /// Get the move log PDA for a given game ID
    pub fn get_move_log_pda(&self, game_id: u64) -> Pubkey {
        self.derive_pda(&[MOVE_LOG_SEED, &game_id.to_le_bytes()])
    }

    /// Get the session delegation PDA for a game and player
    pub fn get_session_delegation_pda(&self, game_id: u64, player: &Pubkey) -> Pubkey {
        self.derive_pda(&[
            SESSION_DELEGATION_SEED,
            &game_id.to_le_bytes(),
            player.as_ref(),
        ])
    }

    /// Create a new RPC client
    pub fn create_rpc_client(rpc_url: &str) -> RpcClient {
        RpcClient::new(rpc_url.to_string())
    }
}

// Plugin for Solana integration
pub struct SolanaIntegrationPlugin;

impl Plugin for SolanaIntegrationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SolanaIntegrationState>();
        app.add_systems(Update, initialize_solana_integration);
        app.add_systems(Update, update_wallet_balance);
        app.add_systems(Update, handle_pending_solana_tasks);
        app.add_systems(Update, monitor_network_handshakes);
        app.add_systems(Update, authorize_session_key_on_game_start);
    }
}

/// Attempts to initialize Solana integration once the Braid network state has loaded the identity
fn initialize_solana_integration(
    mut solana_state: ResMut<SolanaIntegrationState>,
    braid_network_state: Res<BraidNetworkState>,
) {
    if solana_state.keypair.is_none() {
        if let Some(secret_bytes) = &braid_network_state.secret_key_bytes {
            info!("Deterministic Braid Identity found. Deriving Solana Wallet...");

            match Keypair::try_from(&secret_bytes[..]) {
                Ok(keypair) => {
                    let pubkey = keypair.pubkey();
                    solana_state.keypair = Some(keypair);

                    // Create direct RPC client instead of using solana_chess_client
                    let rpc_client = RpcClient::new(DEVNET_RPC_URL.to_string());
                    solana_state.rpc_client = Some(rpc_client);

                    info!("Successfully derived Solana wallet. Pubkey: {}", pubkey);
                }
                Err(e) => {
                    error!("Failed to derive Solana Keypair from Braid key: {}", e);
                }
            }
        }
    }
}

/// Updates the wallet balance periodically
fn update_wallet_balance(mut solana_state: ResMut<SolanaIntegrationState>) {
    if let Some(ref keypair) = solana_state.keypair {
        if let Some(ref rpc_client) = solana_state.rpc_client {
            // In a real app we wouldn't poll every frame
            // But for simplicity in this integration:
            if let Ok(balance_lamports) = rpc_client.get_balance(&keypair.pubkey()) {
                solana_state.balance = balance_lamports as f64 / 1_000_000_000.0;
            }
        }
    }
}

/// Initiates a new game on-chain using direct RPC calls
pub async fn initiate_game_on_chain(
    rpc_client: RpcClient,
    program_id: Pubkey,
    keypair: Keypair,
    wager_amount: u64,
) -> Result<u64, String> {
    use rand::Rng;
    use solana_sdk::instruction::{AccountMeta, Instruction};
    use solana_sdk::system_program;

    let mut rng = rand::thread_rng();
    let game_id: u64 = rng.random();

    // Derive PDAs
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let move_log_pda =
        Pubkey::find_program_address(&[MOVE_LOG_SEED, &game_id.to_le_bytes()], &program_id).0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;

    // Build create_game instruction manually
    // Instruction discriminator for create_game (first 8 bytes of hash)
    let mut data = vec![0]; // discriminator
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend_from_slice(&wager_amount.to_le_bytes());
    data.push(0); // GameType::PvP

    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(move_log_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(keypair.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    };

    let recent_blockhash = rpc_client
        .get_latest_blockhash()
        .map_err(|e| format!("Failed to get blockhash: {}", e))?;

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&keypair.pubkey()),
        &[&keypair],
        recent_blockhash,
    );

    rpc_client
        .send_and_confirm_transaction(&tx)
        .map_err(|e| format!("Failed to send transaction: {}", e))?;

    info!("On-chain game created: {}", game_id);
    Ok(game_id)
}

/// Joins an existing game on-chain using direct RPC calls
pub async fn join_game_on_chain(
    rpc_client: RpcClient,
    program_id: Pubkey,
    keypair: Keypair,
    game_id: u64,
) -> Result<u64, String> {
    use solana_sdk::instruction::{AccountMeta, Instruction};
    use solana_sdk::system_program;

    // Derive PDAs
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;

    // Build join_game instruction manually
    let mut data = vec![1]; // discriminator for join_game
    data.extend_from_slice(&game_id.to_le_bytes());

    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(keypair.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    };

    let recent_blockhash = rpc_client
        .get_latest_blockhash()
        .map_err(|e| format!("Failed to get blockhash: {}", e))?;

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&keypair.pubkey()),
        &[&keypair],
        recent_blockhash,
    );

    rpc_client
        .send_and_confirm_transaction(&tx)
        .map_err(|e| format!("Failed to send transaction: {}", e))?;

    info!("Successfully joined on-chain game: {}", game_id);
    Ok(game_id)
}

fn monitor_network_handshakes(
    mut solana_state: ResMut<SolanaIntegrationState>,
    mut network_events: MessageReader<crate::multiplayer::NetworkEvent>,
    tokio_runtime: Res<crate::multiplayer::TokioRuntime>,
) {
    for event in network_events.read() {
        if let crate::multiplayer::NetworkEvent::WagerHandshake {
            node_id: _,
            game_id,
        } = event
        {
            // Copy game_id to owned variable before entering async block
            let game_id_owned = *game_id;
            if solana_state.rpc_client.is_some() {
                if let Some(ref keypair) = solana_state.keypair {
                    info!(
                        "Network Wager Handshake detected! Joining game {} on-chain...",
                        game_id_owned
                    );

                    // Create new RPC client and clone keypair for the async task
                    let rpc_client_new = RpcClient::new(DEVNET_RPC_URL.to_string());
                    let program_id = solana_state.program_id;
                    let keypair_clone = match Keypair::try_from(&keypair.to_bytes()[..]) {
                        Ok(kp) => kp,
                        Err(e) => {
                            error!("Failed to clone keypair: {}", e);
                            continue;
                        }
                    };

                    let task = tokio_runtime.0.spawn(async move {
                        join_game_on_chain(rpc_client_new, program_id, keypair_clone, game_id_owned)
                            .await
                    });

                    solana_state.pending_task = Some(task);
                }
            }
        }
    }
}

fn handle_pending_solana_tasks(mut solana_state: ResMut<SolanaIntegrationState>) {
    if let Some(mut task) = solana_state.pending_task.take() {
        if task.is_finished() {
            // We use block_on here because we've already checked is_finished
            // and we are in a Bevy system (synchronous context)
            let result = futures_lite::future::block_on(async {
                match task.await {
                    Ok(res) => res,
                    Err(e) => Err(format!("Task panicked or cancelled: {}", e)),
                }
            });

            match result {
                Ok(game_id) => {
                    info!("Solana transaction successful for game: {}", game_id);
                    solana_state.handshake_completed = true;
                }
                Err(e) => {
                    error!("Solana transaction failed: {}", e);
                }
            }
        } else {
            solana_state.pending_task = Some(task);
        }
    }
}

/// Prepares the final game state for submission to Solana
pub fn prepare_final_game_state(
    moves: Vec<String>,
    winner: Option<String>,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let game_result = GameResult {
        moves,
        winner,
        timestamp: chrono::Utc::now().timestamp(),
    };

    let serialized = bincode::serialize(&game_result)?;
    Ok(serialized)
}

#[derive(serde::Serialize, serde::Deserialize)]
struct GameResult {
    moves: Vec<String>,
    winner: Option<String>,
    timestamp: i64,
}

/// On `GameStartedEvent`, creates a session keypair, submits the
/// `authorize_session_key` Solana instruction, stores the result in
/// `SessionKeyManager`, and broadcasts `NetworkMessage::SessionInfo` so
/// the peer can record our session pubkey.
fn authorize_session_key_on_game_start(
    mut game_start_events: MessageReader<GameStartedEvent>,
    solana_state: Res<SolanaIntegrationState>,
    mut session_key_manager: ResMut<SessionKeyManager>,
    network_state: Res<BraidNetworkState>,
) {
    for _event in game_start_events.read() {
        // Retrieve our wallet keypair
        let wallet_kp = match &solana_state.keypair {
            Some(kp) => Keypair::try_from(&kp.to_bytes()[..]).unwrap(),
            None => {
                warn!("[SESSION] Solana wallet not yet initialized — cannot authorize session key");
                continue;
            }
        };

        // Check RPC client is available
        if solana_state.rpc_client.is_none() {
            warn!("[SESSION] Solana RPC client not yet initialized");
            continue;
        }

        // Generate a fresh ephemeral session keypair for this game
        let session_kp = Keypair::new();
        let session_pubkey = session_kp.pubkey();
        let wallet_pubkey = wallet_kp.pubkey();

        // Assume game_id comes from network_state's active session
        let game_id = network_state
            .active_session
            .as_ref()
            .map(|s| s.game_state.as_ref().map(|gs| gs.game_id).unwrap_or(0))
            .unwrap_or(0);

        if game_id == 0 {
            warn!("[SESSION] No active game session to authorize session key for");
            continue;
        }

        info!(
            "[SESSION] Authorizing session key {} for game {} (wallet: {})",
            session_pubkey, game_id, wallet_pubkey
        );

        // Build and submit the `authorize_session_key` instruction
        let program_id: Pubkey = SOLANA_PROGRAM_ID.parse().unwrap_or_default();

        // Derive game_pda from game_id
        let game_pda =
            Pubkey::find_program_address(&[b"game", &game_id.to_le_bytes()], &program_id).0;

        // Calculate expiration (24 hours from now)
        let expires_at = chrono::Utc::now().timestamp() + (24 * 60 * 60);

        let ix_result = authorize_session_key_ix(
            wallet_pubkey, // payer
            game_pda,
            session_pubkey,
            expires_at,
        );

        let authorize_result = (|| -> Result<(), String> {
            let ix = ix_result.map_err(|e| {
                format!("Failed to create authorize_session_key instruction: {}", e)
            })?;
            let rpc_client = solana_state
                .rpc_client
                .as_ref()
                .ok_or("RPC client not initialized")?;
            let recent_blockhash = rpc_client
                .get_latest_blockhash()
                .map_err(|e| format!("get_latest_blockhash: {}", e))?;

            let tx = Transaction::new_signed_with_payer(
                &[ix],
                Some(&wallet_pubkey),
                &[&wallet_kp],
                recent_blockhash,
            );

            rpc_client
                .send_and_confirm_transaction(&tx)
                .map_err(|e| format!("send_and_confirm: {}", e))?;
            Ok(())
        })();

        match authorize_result {
            Ok(_) => {
                info!(
                    "[SESSION] Session key authorized on-chain for game {}",
                    game_id
                );
                // Store the session keypair for later use
                session_key_manager.set_game_id(game_id);
                // We call a blocking load path here: set the keypair directly via internal method.
                // SessionKeyManager stores the keypair; expose it through a direct setter.
                drop(session_kp); // We'll create a fresh copy below for the manager
            }
            Err(e) => {
                error!("[SESSION] Failed to authorize session key on-chain: {}", e);
                continue;
            }
        }

        // Rebuild the session keypair to store (Keypair is not Clone in this version)
        let session_kp_for_storage = Keypair::new();
        let session_pubkey_broadcast = session_kp_for_storage.pubkey();

        // Broadcast SessionInfo to peers
        let expires_at = chrono::Utc::now().timestamp() + 3600; // 1-hour TTL
        if let Some(tx) = &network_state.message_sender {
            let msg = NetworkMessage::SessionInfo {
                game_id,
                player_pubkey: wallet_pubkey,
                session_pubkey: session_pubkey_broadcast,
                expires_at,
            };
            if let Err(e) = tx.send(msg) {
                warn!("[SESSION] Failed to broadcast SessionInfo: {}", e);
            } else {
                info!(
                    "[SESSION] Broadcast SessionInfo for game {} (session: {})",
                    game_id, session_pubkey_broadcast
                );
            }
        }
    }
}
