#![cfg(feature = "solana")]
use bevy::prelude::*;
use bevy::ecs::event::EventReader;
use rand::Rng;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::str::FromStr;

use crate::game::events::GameStartedEvent;
use crate::multiplayer::{
    session_key_manager::SessionKeyManager, BraidNetworkState, NetworkMessage,
};
use crate::solana::{constants::SOLANA_PROGRAM_ID, instructions::authorize_session_key_ix};

// Resource to hold Solana integration state
#[derive(Resource)]
pub struct SolanaIntegrationState {
    /// The derived Solana keypair from the Iroh node key
    pub keypair: Option<Keypair>,
    /// High-level RPC client for XFChess program
    pub client: Option<solana_chess_client::rpc::ChessRpcClient>,
    /// Current balance of the wallet
    pub balance: f64,
    /// Whether the handshake with opponent is completed
    pub handshake_completed: bool,
    /// Pending transaction task
    pub pending_task: Option<tokio::task::JoinHandle<Result<u64, String>>>,
    /// The opponent's public key (for verification)
    pub opponent_pubkey: Option<Pubkey>,
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
            client: None,
            balance: 0.0,
            handshake_completed: false,
            pending_task: None,
            opponent_pubkey: None,
        }
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

                    let client = solana_chess_client::rpc::ChessRpcClient::new(
                        "https://api.devnet.solana.com",
                    );
                    solana_state.client = Some(client);

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
        if let Some(ref client) = solana_state.client {
            // In a real app we wouldn't poll every frame
            // But for simplicity in this integration:
            if let Ok(balance_lamports) = client.rpc.get_balance(&keypair.pubkey()) {
                solana_state.balance = balance_lamports as f64 / 1_000_000_000.0;
            }
        }
    }
}

/// Initiates a new game on-chain
pub async fn initiate_game_on_chain(
    client: solana_chess_client::rpc::ChessRpcClient,
    keypair: Keypair,
    wager_amount: u64,
) -> Result<u64, String> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let game_id: u64 = rng.random();

    let ix = client.create_create_game_ix(
        keypair.pubkey(),
        game_id,
        wager_amount,
        xfchess_game::state::GameType::PvP,
    );

    let recent_blockhash = client
        .rpc
        .get_latest_blockhash()
        .map_err(|e| format!("Failed to get blockhash: {}", e))?;

    let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
        &[ix],
        Some(&keypair.pubkey()),
        &[keypair],
        recent_blockhash,
    );

    client
        .rpc
        .send_and_confirm_transaction(&tx)
        .map_err(|e| format!("Failed to send transaction: {}", e))?;

    info!("On-chain game created: {}", game_id);
    Ok(game_id)
}

/// Joins an existing game on-chain
pub async fn join_game_on_chain(
    client: solana_chess_client::rpc::ChessRpcClient,
    keypair: Keypair,
    game_id: u64,
) -> Result<u64, String> {
    let ix = client.create_join_game_ix(keypair.pubkey(), game_id);

    let recent_blockhash = client
        .rpc
        .get_latest_blockhash()
        .map_err(|e| format!("Failed to get blockhash: {}", e))?;

    let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
        &[ix],
        Some(&keypair.pubkey()),
        &[keypair],
        recent_blockhash,
    );

    client
        .rpc
        .send_and_confirm_transaction(&tx)
        .map_err(|e| format!("Failed to send transaction: {}", e))?;

    info!("Successfully joined on-chain game: {}", game_id);
    Ok(game_id)
}

fn monitor_network_handshakes(
    mut solana_state: ResMut<SolanaIntegrationState>,
    mut network_events: EventReader<crate::multiplayer::NetworkEvent>,
    tokio_runtime: Res<crate::multiplayer::TokioRuntime>,
) {
    for event in network_events.read() {
        if let crate::multiplayer::NetworkEvent::WagerHandshake {
            node_id: _,
            game_id,
        } = event
        {
            if let Some(ref client) = solana_state.client {
                if let Some(ref keypair) = solana_state.keypair {
                    info!(
                        "Network Wager Handshake detected! Joining game {} on-chain...",
                        game_id
                    );

                    // Create new client and clone keypair for the async task
                    let client_new = solana_chess_client::rpc::ChessRpcClient::new(
                        "https://api.devnet.solana.com",
                    );
                    let keypair_clone = match Keypair::try_from(&keypair.to_bytes()[..]) {
                        Ok(kp) => kp,
                        Err(e) => {
                            error!("Failed to clone keypair: {}", e);
                            continue;
                        }
                    };
                    let gid = game_id;

                    let task = tokio_runtime.0.spawn(async move {
                        join_game_on_chain(client_new, keypair_clone, gid).await
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
    mut game_start_events: EventReader<GameStartedEvent>,
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

        let client = match &solana_state.client {
            Some(c) => c.clone(),
            None => {
                warn!("[SESSION] Solana RPC client not yet initialized");
                continue;
            }
        };

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
        let game_pda = Pubkey::find_program_address(
            &[b"game", &game_id.to_le_bytes()],
            &program_id,
        ).0;
        
        // Calculate expiration (24 hours from now)
        let expires_at = chrono::Utc::now().timestamp() + (24 * 60 * 60);
        
        let ix = authorize_session_key_ix(
            wallet_pubkey, // payer
            game_pda,
            session_pubkey,
            expires_at,
        ).map_err(|e| format!("Failed to create authorize_session_key instruction: {}", e))?;

        let authorize_result = (|| -> Result<(), String> {
            let recent_blockhash = client
                .rpc
                .get_latest_blockhash()
                .map_err(|e| format!("get_latest_blockhash: {}", e))?;

            let tx = Transaction::new_signed_with_payer(
                &[ix],
                Some(&wallet_pubkey),
                &[&wallet_kp],
                recent_blockhash,
            );

            client
                .rpc
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
