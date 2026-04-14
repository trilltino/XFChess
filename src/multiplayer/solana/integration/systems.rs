use super::state::{BalanceRefreshTimer, SolanaIntegrationState, DEVNET_RPC_URL};
use bevy::prelude::{debug, error, info, warn, Local, Res, ResMut, Time};
use bevy::ecs::message::MessageReader;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use crate::game::events::GameStartedEvent;
use crate::multiplayer::{BraidNetworkState, NetworkMessage};
use crate::multiplayer::solana::session_key_manager::SessionKeyManager;
use std::sync::Arc;
use directories::ProjectDirs;
use std::path::PathBuf;
use std::fs;

pub fn initialize_solana_integration(
    mut solana_state: ResMut<SolanaIntegrationState>,
    mut solana_wallet: Option<ResMut<crate::multiplayer::solana::addon::SolanaWallet>>,
    tokio_runtime: Res<crate::multiplayer::TokioRuntime>,
    time: Res<Time>,
    mut rx: Local<Option<crossbeam_channel::Receiver<Option<String>>>>,
    mut retry_secs: Local<f32>,
) {
    if solana_state.wallet_pubkey.is_some() {
        return;
    }

    if let Some(ref receiver) = *rx {
        match receiver.try_recv() {
            Ok(Some(pubkey_str)) => {
                *rx = None;
                
                // 1. Handle sentinel/non-pubkey strings first to avoid Base58 parsing noise
                let trimmed = pubkey_str.trim();
                if trimmed.is_empty() || trimmed == "undefined" || trimmed == "null" {
                    debug!("[WALLET] Transit state: {}", trimmed);
                    *retry_secs = 2.0;
                } else if trimmed == "hot-wallet-dummy" {
                    info!("[WALLET] Defaulting to local hot wallet as requested by Tauri.");
                    if let Some(keypair) = load_or_create_hot_wallet() {
                        let pubkey = keypair.pubkey();
                        info!("[WALLET] Hot wallet initialized. Pubkey: {}", pubkey);
                        solana_state.wallet_pubkey = Some(pubkey);
                        solana_state.rpc_client = Some(RpcClient::new(DEVNET_RPC_URL.to_string()));
                        if let Some(ref mut w) = solana_wallet {
                            w.pubkey = Some(pubkey);
                            w.keypair = Some(Arc::new(keypair));
                        }
                    }
                } else {
                    // 2. Attempt to parse as real Solana Pubkey
                    match trimmed.parse::<Pubkey>() {
                        Ok(pubkey) => {
                            info!("[WALLET] Phantom wallet connected. Pubkey: {}", pubkey);
                            solana_state.wallet_pubkey = Some(pubkey);
                            solana_state.rpc_client =
                                Some(RpcClient::new(DEVNET_RPC_URL.to_string()));
                            if let Some(ref mut w) = solana_wallet {
                                w.pubkey = Some(pubkey);
                            }

                            // Try to load existing session key
                            match SessionKeyManager::load_session(&pubkey) {
                                Ok(session_manager) => {
                                    info!("[SESSION] Loaded existing session key: {}", session_manager.pubkey());
                                    solana_state.session_keypair = Some(solana_sdk::signature::Keypair::try_from(session_manager.signer().to_bytes().as_slice()).unwrap());
                                }
                                Err(e) => {
                                    info!("[SESSION] No valid session key found ({}), will create new one", e);
                                    // Create new session key
                                    let session_manager = SessionKeyManager::new(&pubkey);
                                    let session_pubkey = session_manager.pubkey();
                                    solana_state.session_keypair = Some(solana_sdk::signature::Keypair::try_from(session_manager.signer().to_bytes().as_slice()).unwrap());
                                    
                                    // Save session data (24 hour default)
                                    if let Err(e) = session_manager.save_session(&pubkey, 24) {
                                        warn!("[SESSION] Failed to save session key: {}", e);
                                    }
                                    
                                    info!("[SESSION] Created new session key: {}", session_pubkey);
                                    
                                    // Authorize session key on-chain (async)
                                    let _rpc_client = RpcClient::new(DEVNET_RPC_URL.to_string());
                                    let _session_pubkey_clone = session_pubkey;
                                    let _pubkey_clone = pubkey;
                                    tokio_runtime.0.spawn(async move {
                                        // Note: We can't sign without the wallet keypair here
                                        // In production, this would be done via Tauri wallet popup
                                        info!("[SESSION] Session key authorization requires wallet signature (deferred to first transaction)");
                                    });
                                }
                            }
                        }
                        Err(e) => {
                            warn!("[WALLET] Invalid pubkey from Tauri: {} (Raw: '{}')", e, trimmed);
                            *retry_secs = 2.0;
                        }
                    }
                }
            }
            Ok(None) => {
                *rx = None;
                *retry_secs = 2.0; 
            }
            Err(crossbeam_channel::TryRecvError::Disconnected) => {
                *rx = None;
                *retry_secs = 3.0; // Wait longer on host failure
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {} 
        }
        return;
    }

    if *retry_secs > 0.0 {
        *retry_secs -= time.delta_secs();
        return;
    }

    let (tx, receiver) = crossbeam_channel::bounded::<Option<String>>(1);
    *rx = Some(receiver);
    tokio_runtime.0.spawn(async move {
        let _ = tx.send(query_wallet_pubkey_from_tauri().await);
    });
}

pub async fn query_wallet_pubkey_from_tauri() -> Option<String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    let base: u16 = std::env::var("XFCHESS_WALLET_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(7454);
    let tcp_start = base.saturating_sub(11);
    let tcp_end = base.saturating_sub(2);
    for port in tcp_start..=tcp_end {
        let mut stream = match TcpStream::connect(("127.0.0.1", port)).await {
            Ok(s) => s,
            Err(_) => continue,
        };
        let _ = stream.write_all(b"PKEY").await;
        let mut len_buf = [0u8; 4];
        if stream.read_exact(&mut len_buf).await.is_err() {
            return None;
        }
        let len = u32::from_le_bytes(len_buf) as usize;
        if len == 0 {
            return None; 
        }
        let mut buf = vec![0u8; len];
        if stream.read_exact(&mut buf).await.is_err() {
            return None;
        }
        return String::from_utf8(buf).ok();
    }
    None
}

pub fn update_wallet_balance(
    mut solana_state: ResMut<SolanaIntegrationState>,
    mut timer: ResMut<BalanceRefreshTimer>,
    time: Res<Time>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }
    let (Some(ref pubkey), Some(ref rpc_client)) =
        (solana_state.wallet_pubkey.as_ref(), solana_state.rpc_client.as_ref())
    else {
        return;
    };
    match rpc_client.get_balance(*pubkey) {
        Ok(lamports) => solana_state.balance = lamports as f64 / 1_000_000_000.0,
        Err(e) => warn!("[SOLANA] Balance fetch failed: {}", e),
    }
}

pub fn monitor_network_handshakes(
    mut solana_state: ResMut<SolanaIntegrationState>,
    mut network_events: MessageReader<crate::multiplayer::NetworkEvent>,
) {
    for event in network_events.read() {
        if let crate::multiplayer::NetworkEvent::WagerHandshake {
            node_id: _,
            game_id,
        } = event
        {
            let game_id_owned = *game_id;
            let wallet_pubkey = match solana_state.wallet_pubkey {
                Some(pk) => pk,
                None => {
                    warn!("[HANDSHAKE] Wallet not connected — cannot join game {} on-chain", game_id_owned);
                    continue;
                }
            };

            info!("[HANDSHAKE] Wager handshake for game {} — joining on-chain via Phantom", game_id_owned);

            let program_id = solana_state.program_id;

            let task = bevy::tasks::IoTaskPool::get().spawn(async move {
                use crate::multiplayer::solana::tauri_signer::sign_and_send_via_tauri;
                use crate::solana::instructions::join_game_ix;

                let ix = join_game_ix(program_id, wallet_pubkey, game_id_owned)
                    .map_err(|e| format!("build join_game_ix: {}", e))?;

                sign_and_send_via_tauri(DEVNET_RPC_URL, wallet_pubkey, &[ix], &[])
                    .map(|_sig| game_id_owned)
                    .map_err(|e| format!("join_game sign: {}", e))
            });

            task.detach();
            solana_state.handshake_completed = false;
        }
    }
}

pub fn handle_pending_solana_tasks(mut solana_state: ResMut<SolanaIntegrationState>) {
    if let Some(task) = solana_state.pending_task.take() {
        if task.is_finished() {
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

pub fn authorize_session_key_on_game_start(
    mut game_start_events: MessageReader<GameStartedEvent>,
    solana_state: Res<SolanaIntegrationState>,
    network_state: Res<BraidNetworkState>,
    rollup_manager: Res<crate::multiplayer::rollup::manager::EphemeralRollupManager>,
) {
    for _event in game_start_events.read() {
        let wallet_pubkey = match solana_state.wallet_pubkey {
            Some(pk) => pk,
            None => {
                warn!("[SESSION] Wallet not connected");
                continue;
            }
        };

        let game_id = rollup_manager.game_id;

        if game_id == 0 {
            warn!("[SESSION] No active game_id for session broadcast");
            continue;
        }

        let msg_sender = network_state.message_sender.clone();

        bevy::tasks::IoTaskPool::get()
            .spawn(async move {
                use crate::multiplayer::vps_client;

                let mut active = false;
                for _ in 0..60 {
                    match vps_client::session_status(game_id) {
                        Ok(s) if s.active => {
                            active = true;
                            let session_pubkey: Pubkey = match s.session_pubkey.parse() {
                                Ok(pk) => pk,
                                Err(_) => break,
                            };
                            info!("[SESSION] VPS session active for game {} ({})", game_id, session_pubkey);

                            let expires_at = chrono::Utc::now().timestamp() + 3600;
                            if let Some(ref tx) = msg_sender {
                                let msg = NetworkMessage::SessionInfo {
                                    game_id,
                                    player_pubkey: wallet_pubkey,
                                    session_pubkey,
                                    expires_at,
                                };
                                let _ = tx.send(msg);
                            }
                            break;
                        }
                        Ok(_) => std::thread::sleep(std::time::Duration::from_secs(1)),
                        Err(e) => {
                            warn!("[SESSION] VPS status poll error: {e}");
                            std::thread::sleep(std::time::Duration::from_secs(1));
                        }
                    }
                }
                if !active {
                    error!("[SESSION] VPS session never became active for game {}", game_id);
                }
            })
            .detach();
    }
}

/// Resolves the storage path for the local hot wallet
fn get_hot_wallet_path() -> Option<PathBuf> {
    ProjectDirs::from("com", "trilltino", "XFChess").map(|proj_dirs| {
        let config_dir = proj_dirs.config_dir();
        config_dir.join("hot_wallet.json")
    })
}

/// Loads an existing hot wallet or generates a new one
fn load_or_create_hot_wallet() -> Option<Keypair> {
    let path = get_hot_wallet_path()?;

    if path.exists() {
        match fs::read_to_string(&path) {
            Ok(contents) => {
                // Try to parse as JSON byte array (standard solana-keygen format)
                match serde_json::from_str::<Vec<u8>>(&contents) {
                    Ok(bytes) => match Keypair::try_from(bytes.as_slice()) {
                        Ok(kp) => return Some(kp),
                        Err(e) => error!("[WALLET] Failed to parse keypair from bytes: {}", e),
                    },
                    Err(_) => {
                        // Try to parse as raw Base58 string
                        // from_base58_string returns Keypair directly, not Result
                        let kp = Keypair::from_base58_string(contents.trim());
                        return Some(kp);
                    }
                }
            }
            Err(e) => error!("[WALLET] Failed to read hot wallet file: {}", e),
        }
    }

    // Generate new keypair if loading failed or file didn't exist
    info!("[WALLET] Generating new local hot wallet...");
    let new_kp = Keypair::new();
    
    // Save to disk
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let bytes = new_kp.to_bytes().to_vec();
    if let Ok(json) = serde_json::to_string(&bytes) {
        if let Err(e) = fs::write(&path, json) {
            error!("[WALLET] Failed to save hot wallet: {}", e);
        } else {
            info!("[WALLET] Saved new hot wallet to {:?}", path);
        }
    }

    Some(new_kp)
}
