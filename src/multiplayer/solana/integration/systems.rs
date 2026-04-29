use super::state::{BalanceRefreshTimer, SolanaIntegrationState, DEVNET_RPC_URL};
use bevy::prelude::{debug, error, info, warn, Local, Res, ResMut, Time, Commands};
use bevy::ecs::message::MessageReader;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use crate::game::events::GameStartedEvent;
use crate::core::GameState;
use crate::multiplayer::{BraidNetworkState, NetworkMessage};
use crate::multiplayer::solana::session_key_manager::SessionKeyManager;
use crate::multiplayer::solana::tournament::TournamentClientState;
use crate::multiplayer::vps_client::UserStatus;
use std::sync::Arc;
use directories::ProjectDirs;
use std::path::PathBuf;
use std::fs;

/// Solana RPC configuration with relayer fee payer
#[derive(Clone, Debug, bevy::prelude::Resource)]
pub struct SolanaRpc {
    pub rpc_url: String,
    pub fee_payer: String,
}

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
                                    solana_state.session_keypair = Some(solana_sdk::signature::Keypair::from_bytes(&session_manager.signer().to_bytes()).unwrap());
                                }
                                Err(e) => {
                                    info!("[SESSION] No valid session key found ({}), will create new one", e);
                                    // Create new session key
                                    let session_manager = SessionKeyManager::new(&pubkey);
                                    let session_pubkey = session_manager.pubkey();
                                    solana_state.session_keypair = Some(solana_sdk::signature::Keypair::from_bytes(&session_manager.signer().to_bytes()).unwrap());
                                    
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
        let _ = tx.send(query_wallet_pubkey_from_tauri());
    });
}

pub fn query_wallet_pubkey_from_tauri() -> Option<String> {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    let base: u16 = std::env::var("XFCHESS_WALLET_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(7454);
    let tcp_start = base.saturating_sub(11);
    let tcp_end = base.saturating_sub(2);
    for port in tcp_start..=tcp_end {
        let mut stream = match TcpStream::connect(("127.0.0.1", port)) {
            Ok(s) => s,
            Err(_) => continue,
        };
        stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
        let _ = stream.write_all(b"PKEY");
        let mut len_buf = [0u8; 4];
        if stream.read_exact(&mut len_buf).is_err() {
            return None;
        }
        let len = u32::from_le_bytes(len_buf) as usize;
        if len == 0 {
            return None; 
        }
        let mut buf = vec![0u8; len];
        if stream.read_exact(&mut buf).is_err() {
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
    mut popup_queue: ResMut<crate::ui::menus::popup::GamePopupQueue>,
) {
    for event in network_events.read() {
        if let crate::multiplayer::NetworkEvent::WagerHandshake {
            node_id: _,
            game_id,
        } = event
        {
            // Push "Check Wallet" notification
            popup_queue.push(crate::ui::menus::popup::GamePopup {
                title: "Confirm Wager".to_string(),
                message: "A wager match is starting. Please confirm the wager transaction in your wallet.".to_string(),
                copy_text: None,
                url: None,
                url_label: None,
                lifetime: 15.0,
                remaining: 15.0,
                dismissed: false,
            });
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

                let ix = join_game_ix(
                    program_id,
                    wallet_pubkey,
                    wallet_pubkey, // white_player — will be resolved properly via lobby flow
                    wallet_pubkey, // fee_payer — placeholder; session key used in real path
                    game_id_owned,
                )
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
                    Ok(bytes) => match Keypair::from_bytes(&bytes) {
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

/// Fetches user verification status from VPS backend and caches it in SolanaWallet
/// Runs periodically (every 30s) when wallet is connected
pub fn fetch_user_status(
    solana_wallet: Option<Res<crate::multiplayer::solana::addon::SolanaWallet>>,
    solana_wallet_mut: Option<ResMut<crate::multiplayer::solana::addon::SolanaWallet>>,
    time: Res<Time>,
    mut timer: Local<f32>,
) {
    *timer -= time.delta_secs();
    if *timer > 0.0 {
        return;
    }
    *timer = 30.0; // Refresh every 30 seconds

    let (wallet, _wallet_mut) = match (solana_wallet, solana_wallet_mut) {
        (Some(w), Some(wm)) => (w, wm),
        _ => return,
    };

    let pubkey = match wallet.pubkey {
        Some(pk) => pk.to_string(),
        None => return,
    };

    let pubkey_clone = pubkey.clone();
    let pubkey_display = pubkey.clone();
    // Spawn async task to fetch status
    bevy::tasks::IoTaskPool::get().spawn(async move {
        match crate::multiplayer::vps_client::get_user_status_async(pubkey_clone).await {
            Ok(status) => {
                info!("[USER_STATUS] Fetched status for {}: profile={}, email={}, kyc={}, can_wager={}",
                    pubkey_display, status.has_profile, status.has_email, status.has_kyc, status.can_wager);
                // Note: We can't write to ResMut from async task, so this is a placeholder
                // In production, use a channel or event to communicate back to main thread
            }
            Err(e) => {
                warn!("[USER_STATUS] Failed to fetch status for {}: {}", pubkey_display, e);
            }
        }
    }).detach();
}

/// Alternative version that uses a channel-based approach for async-to-main communication
/// This is the preferred pattern for Bevy
pub fn fetch_user_status_async(
    solana_wallet: Option<Res<crate::multiplayer::solana::addon::SolanaWallet>>,
    mut solana_wallet_mut: Option<ResMut<crate::multiplayer::solana::addon::SolanaWallet>>,
    time: Res<Time>,
    mut timer: Local<f32>,
    tokio_runtime: Res<crate::multiplayer::TokioRuntime>,
) {
    *timer -= time.delta_secs();
    if *timer > 0.0 {
        return;
    }
    *timer = 30.0; // Refresh every 30 seconds

    let pubkey = match solana_wallet.as_ref().and_then(|w| w.pubkey) {
        Some(pk) => pk.to_string(),
        None => return,
    };

    let pubkey_display = pubkey.clone();
    let (tx, rx) = crossbeam_channel::bounded::<Option<UserStatus>>(1);
    let pubkey_clone = pubkey.clone();
    tokio_runtime.0.spawn(async move {
        let result = crate::multiplayer::vps_client::get_user_status_async(pubkey_clone).await.ok();
        let _ = tx.send(result);
    });

    // Try to receive immediately (non-blocking)
    if let Ok(Some(status)) = rx.try_recv() {
        if let Some(ref mut w) = solana_wallet_mut {
            w.user_status = Some(status);
            info!("[USER_STATUS] Updated cached status for {}", pubkey_display);
        }
    }
}

/// Syncs own and opponent profiles from VPS when a competitive match starts
pub fn sync_player_profiles(
    mut competitive: ResMut<crate::multiplayer::solana::addon::CompetitiveMatchState>,
    mut profile: ResMut<crate::multiplayer::solana::addon::SolanaProfile>,
    solana_state: Res<SolanaIntegrationState>,
    tokio_runtime: Res<crate::multiplayer::TokioRuntime>,
    mut own_rx: Local<Option<crossbeam_channel::Receiver<Option<crate::multiplayer::network::vps::PlayerProfile>>>>,
    mut opp_rx: Local<Option<crossbeam_channel::Receiver<Option<crate::multiplayer::network::vps::PlayerProfile>>>>,
    mut last_game_id: Local<Option<u64>>,
    mut last_opp_pk: Local<Option<Pubkey>>,
) {
    // Trigger fetches
    if competitive.active {
        if competitive.game_id != *last_game_id || competitive.opponent_pubkey != *last_opp_pk {
            *last_game_id = competitive.game_id;
            *last_opp_pk = competitive.opponent_pubkey;

            // Fetch own
            if let Some(pk) = solana_state.wallet_pubkey {
                let (tx, rx) = crossbeam_channel::bounded(1);
                let pk_str = pk.to_string();
                tokio_runtime.0.spawn(async move {
                    let _ = tx.send(crate::multiplayer::network::vps::fetch_player_profile(&pk_str).ok());
                });
                *own_rx = Some(rx);
            }

            // Fetch opponent
            if let Some(pk) = competitive.opponent_pubkey {
                let (tx, rx) = crossbeam_channel::bounded(1);
                let pk_str = pk.to_string();
                tokio_runtime.0.spawn(async move {
                    let _ = tx.send(crate::multiplayer::network::vps::fetch_player_profile(&pk_str).ok());
                });
                *opp_rx = Some(rx);
            }
        }
    } else {
        *last_game_id = None;
        *last_opp_pk = None;
    }

    // Poll results
    if let Some(ref rx) = *own_rx {
        if let Ok(res) = rx.try_recv() {
            if let Some(p) = res {
                profile.elo = p.elo;
                profile.username = p.username;
                profile.country = p.country;
                info!("[PROFILES] Updated own profile: {} ({} ELO)", profile.username, profile.elo);
            }
            *own_rx = None;
        }
    }

    if let Some(ref rx) = *opp_rx {
        if let Ok(res) = rx.try_recv() {
            if let Some(p) = res {
                competitive.opponent_elo = p.elo;
                competitive.opponent_username = p.username;
                competitive.opponent_country = p.country;
                info!("[PROFILES] Updated opponent profile: {} ({} ELO)", competitive.opponent_username, competitive.opponent_elo);
            }
            *opp_rx = None;
        }
    }
}

pub fn setup_solana_system(
    mut commands: Commands,
) {
    // Placeholder for fetching relayer_pubkey from backend or environment
    let relayer_pubkey = "PlaceholderRelayerPubkey";
    commands.insert_resource(SolanaRpc {
        rpc_url: "https://api.devnet.solana.com".to_string(),
        fee_payer: relayer_pubkey.to_string(),
    });
}

pub fn handle_game_transactions(
    _game_state: ResMut<GameState>,
    _solana_rpc: Res<SolanaRpc>,
) {
    // Use solana_rpc.fee_payer for transactions
    // Placeholder for transaction logic
}

pub fn handle_tournament_transactions(
    _tournament_state: ResMut<TournamentClientState>,
    _solana_rpc: Res<SolanaRpc>,
) {
    // Use solana_rpc.fee_payer for tournament transactions
    // Placeholder for transaction logic
}
