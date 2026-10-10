use super::state::{BalanceRefreshTimer, SolanaIntegrationState, DEVNET_RPC_URL};
use crate::core::GameState;
use crate::game::events::GameStartedEvent;
use crate::multiplayer::solana::tournament::TournamentClientState;
use crate::multiplayer::vps_client::UserStatus;
use crate::multiplayer::{NetworkMessage, OnlineNetworkState};
use bevy::ecs::message::MessageReader;
use bevy::prelude::{debug, error, info, warn, Commands, Local, Res, ResMut, Time};
#[cfg(not(target_os = "android"))]
use directories::ProjectDirs;
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

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
    mut commands: Commands,
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
                        solana_state.rpc_client = Some(RpcClient::new_with_commitment(
                            DEVNET_RPC_URL.to_string(),
                            CommitmentConfig::confirmed(),
                        ));
                        if let Some(ref mut w) = solana_wallet {
                            w.pubkey = Some(pubkey);
                            w.keypair = Some(Arc::new(keypair));
                        }
                    }
                } else {
                    match trimmed.parse::<Pubkey>() {
                        Ok(pubkey) => {
                            info!("[WALLET] Phantom wallet connected. Pubkey: {}", pubkey);
                            solana_state.wallet_pubkey = Some(pubkey);
                            solana_state.rpc_client = Some(RpcClient::new_with_commitment(
                                DEVNET_RPC_URL.to_string(),
                                CommitmentConfig::confirmed(),
                            ));
                            crate::multiplayer::network::vps::emit_client_event(
                                crate::multiplayer::network::vps::ClientEvent::new(
                                    "solana_wallet_connected",
                                )
                                .wallet(pubkey)
                                .session_kind("wallet"),
                            );
                            if let Some(ref mut w) = solana_wallet {
                                w.pubkey = Some(pubkey);
                            }

                            // Load saved sessions optimistically; backend registration validates them
                            // against on-chain state and may deactivate a stale key.
                            solana_state.try_load_global_session(&pubkey);
                            // Register global sessions on every connect to restore the backend's
                            // in-memory registry and verify the locally saved key.
                            if let Some(ref kp) = solana_state.global_session_keypair {
                                let kp_bytes = kp.to_bytes();
                                let (tx, rx) = crossbeam_channel::bounded(1);
                                std::thread::spawn(move || {
                                    let outcome =
                                        register_global_session_with_backend(pubkey, kp_bytes);
                                    let _ = tx.send(outcome);
                                });
                                commands.insert_resource(GlobalSessionRegisterPending { rx });
                            }

                        }
                        Err(e) => {
                            warn!(
                                "[WALLET] Invalid pubkey from Tauri: {} (Raw: '{}')",
                                e, trimmed
                            );
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
    use crate::multiplayer::solana::tauri_signer::candidate_ports;
    use std::io::{Read, Write};
    use std::net::TcpStream;

    // Prefer the announced bridge port; scan only as a fallback through the shared helper.
    for port in candidate_ports() {
        let mut stream = match TcpStream::connect(("127.0.0.1", port)) {
            Ok(s) => s,
            Err(_) => continue,
        };
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .ok();
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
    tokio_runtime: Res<crate::multiplayer::TokioRuntime>,
    mut rx: Local<Option<crossbeam_channel::Receiver<Result<u64, String>>>>,
    mut last_pubkey: Local<Option<Pubkey>>,
) {
    if let Some(ref receiver) = *rx {
        match receiver.try_recv() {
            Ok(Ok(lamports)) => {
                let sol = lamports as f64 / 1_000_000_000.0;
                solana_state.balance = sol;
                if let Some(rate) = solana_state.sol_usd_rate {
                    solana_state.cached_usd_balance = Some(sol * rate);
                }
                *rx = None;
            }
            Ok(Err(e)) => {
                warn!("[SOLANA] Balance fetch failed: {}", e);
                *rx = None;
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {}
            Err(_) => {
                *rx = None;
            }
        }
        return;
    }

    // Fetch balance immediately on connection so wager eligibility does not
    // wait for the periodic timer's first tick.
    let just_connected = solana_state.wallet_pubkey != *last_pubkey;
    *last_pubkey = solana_state.wallet_pubkey;

    if !just_connected && !timer.0.tick(time.delta()).just_finished() {
        return;
    }
    if just_connected {
        timer.0.reset();
    }
    let (Some(pubkey), Some(rpc_url)) = (
        solana_state.wallet_pubkey,
        solana_state.rpc_client.as_ref().map(|c| c.url()),
    ) else {
        return;
    };

    let (tx, receiver) = crossbeam_channel::bounded(1);
    *rx = Some(receiver);
    tokio_runtime.0.spawn_blocking(move || {
        let rpc = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());
        let _ = tx.send(rpc.get_balance(&pubkey).map_err(|e| e.to_string()));
    });
}

pub fn update_wallet_usd_rate(
    mut solana_state: ResMut<SolanaIntegrationState>,
    tokio_runtime: Res<crate::multiplayer::TokioRuntime>,
    time: Res<Time>,
    mut timer: Local<f32>,
    mut rx: Local<Option<crossbeam_channel::Receiver<Result<f64, String>>>>,
) {
    *timer -= time.delta_secs();

    if let Some(ref receiver) = *rx {
        match receiver.try_recv() {
            Ok(Ok(rate)) => {
                solana_state.sol_usd_rate = Some(rate);
                solana_state.cached_usd_balance = Some(solana_state.balance * rate);
                // No per-refresh log — see `wager_rate.rs`'s matching comment.
                *rx = None;
            }
            Ok(Err(e)) => {
                warn!("[SOLANA] USD rate fetch failed: {}", e);
                *rx = None;
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {}
            Err(_) => {
                *rx = None;
            }
        }
        return;
    }

    if *timer > 0.0 {
        return;
    }
    *timer = 60.0;

    if solana_state.wallet_pubkey.is_none() {
        return;
    }

    let (tx, receiver) = crossbeam_channel::bounded(1);
    *rx = Some(receiver);

    tokio_runtime.0.spawn(async move {
        let _ = tx.send(fetch_sol_usd_rate().await);
    });
}

async fn fetch_sol_usd_rate() -> Result<f64, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| format!("HTTP client build error: {}", e))?;

    let url = format!(
        "{}/api/rates/all",
        crate::multiplayer::network::vps::vps_base()
    );

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("backend rates fetch error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("backend rates HTTP {}", resp.status()));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("backend rates JSON parse error: {}", e))?;

    let price = json["rates"]["usd"]
        .as_f64()
        .ok_or("Missing rates.usd in backend response")?;

    Ok(price)
}

pub fn sync_session_key_to_network(
    mut solana_state: ResMut<SolanaIntegrationState>,
    mut network_state: ResMut<OnlineNetworkState>,
) {
    if network_state.session_signing_key.is_some() {
        return;
    }
    if solana_state.wallet_pubkey.is_none() {
        // Casual play still requires signed gossip. Use the persistent device key
        // when no wallet is connected.
        let node_key = crate::multiplayer::network::identity::load_or_create();
        let sk: [u8; 32] = node_key.to_bytes();
        network_state.session_signing_key = Some(sk);
        if let Ok(mut shared) = network_state.session_signing_key_shared.write() {
            *shared = Some(sk);
        }
        info!(
            "[SESSION] No wallet connected — using persistent node identity as gossip-signing key"
        );
        return;
    }
    if solana_state.session_keypair.is_none() {
        // Persisted per wallet (see `network::device_id::wallet_gossip_seed`)
        // so a restarted client keeps the signer its opponent already knows.
        let wallet = solana_state
            .wallet_pubkey
            .map(|pk| pk.to_string())
            .unwrap_or_default();
        let session_kp = solana_sdk::signature::Keypair::new_from_array(
            crate::multiplayer::network::device_id::wallet_gossip_seed(&wallet),
        );
        info!(
            "[SESSION] Generated gossip-signing key: {}",
            session_kp.pubkey()
        );
        solana_state.session_keypair = Some(session_kp);
    }
    if let Some(ref kp) = solana_state.session_keypair {
        let bytes = kp.to_bytes();
        let mut sk = [0u8; 32];
        sk.copy_from_slice(&bytes[..32]);
        network_state.session_signing_key = Some(sk);
        // The outgoing-message task reads this shared cell, not the plain
        // field above — see its doc comment.
        if let Ok(mut shared) = network_state.session_signing_key_shared.write() {
            *shared = Some(sk);
        }
        info!("[SESSION] Copied session signing key to P2P network state");
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
    mut game_sync: ResMut<crate::multiplayer::solana::addon::SolanaGameSync>,
    network_state: Res<OnlineNetworkState>,
    rollup_manager: Res<crate::multiplayer::rollup::manager::EphemeralRollupManager>,
    braid: Res<crate::multiplayer::network::braid_transport::BraidTransportState>,
) {
    let session_pubkey_update = game_sync.session_pubkey_update.clone();
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
        // Send SessionInfo through both transports so opponent identity survives
        // a missing gossip link and remains available for finalization.
        let node_b58 = network_state
            .node_id
            .as_ref()
            .map(|id| bs58::encode(id.as_bytes()).into_string());
        // Derive the gossip signer public key from the 32-byte Ed25519 seed. The
        // participant roster must match verified message signers, not session delegation keys.
        let signing_pubkey_bytes = network_state.session_signing_key;
        let braid_heads = braid.heads();

        bevy::tasks::IoTaskPool::get()
                .spawn({
                    let session_pubkey_update = session_pubkey_update.clone();
                    async move {
                use crate::multiplayer::vps_client;

                let Some(signing_pubkey) = signing_pubkey_bytes.map(|seed| {
                    use ed25519_dalek::SigningKey;
                    Pubkey::new_from_array(SigningKey::from_bytes(&seed).verifying_key().to_bytes())
                }) else {
                    warn!(
                        "[SESSION] No gossip-signing key yet for game {} — cannot broadcast SessionInfo (moves would be rejected as non-participant on the peer's roster check)",
                        game_id
                    );
                    return;
                };

                let mut active = false;
                for _ in 0..60 {
                    match vps_client::session_status(game_id) {
                        Ok(s) if s.active => {
                            active = true;
                            let session_pubkey: Pubkey = match s.session_pubkey.parse() {
                                Ok(pk) => pk,
                                Err(_) => break,
                            };
                            if let Ok(mut update) = session_pubkey_update.lock() {
                                *update = Some(session_pubkey);
                            }
                            info!(
                                "[SESSION] VPS session active for game {} ({})",
                                game_id, session_pubkey
                            );

                            let expires_at = chrono::Utc::now().timestamp() + 3600;
                            let msg = NetworkMessage::SessionInfo {
                                game_id,
                                player_pubkey: wallet_pubkey,
                                session_pubkey,
                                signing_pubkey,
                                expires_at,
                            };

                            // Publish SessionInfo on both transports and chain it from the shared
                            // stream head; both players write to that stream.
                            crate::multiplayer::network::braid_transport::publish_session_info(
                                crate::multiplayer::network::vps::vps_base(),
                                game_id.to_string(),
                                node_b58.clone().unwrap_or_default(),
                                String::new(),
                                wallet_pubkey.to_string(),
                                session_pubkey.to_string(),
                                signing_pubkey.to_string(),
                                expires_at,
                                braid_heads.clone(),
                            );
                            if let Some(ref tx) = msg_sender {
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
                    error!(
                        "[SESSION] VPS session never became active for game {}",
                        game_id
                    );
                }
                }
            })
            .detach();
    }
}

pub fn poll_session_pubkey_update(
    mut game_sync: ResMut<crate::multiplayer::solana::addon::SolanaGameSync>,
) {
    let session_pubkey = game_sync
        .session_pubkey_update
        .lock()
        .ok()
        .and_then(|mut update| update.take());
    if let Some(session_pubkey) = session_pubkey {
        game_sync.session_pubkey = Some(session_pubkey);
    }
}

pub fn spawn_verified_participants_fetch(
    mut game_start_events: MessageReader<GameStartedEvent>,
    mut solana_state: ResMut<SolanaIntegrationState>,
    tokio_runtime: Res<crate::multiplayer::TokioRuntime>,
    causal: Res<crate::multiplayer::types::CausalChainState>,
) {
    for event in game_start_events.read() {
        let game_id = event.game_id;
        if causal.verified_wallets.contains_key(&game_id) {
            continue;
        }
        if matches!(&solana_state.pending_participants_fetch, Some((gid, _)) if *gid == game_id) {
            continue;
        }

        // fetch_verified_participants uses blocking reqwest; run it on spawn_blocking.
        let handle = tokio_runtime.0.spawn_blocking(move || {
            crate::multiplayer::vps_client::fetch_verified_participants(game_id)
        });
        solana_state.pending_participants_fetch = Some((game_id, handle));
    }
}

pub fn poll_verified_participants_fetch(
    mut solana_state: ResMut<SolanaIntegrationState>,
    mut causal: ResMut<crate::multiplayer::types::CausalChainState>,
) {
    let Some((_, task)) = &solana_state.pending_participants_fetch else {
        return;
    };
    if !task.is_finished() {
        return;
    }
    let (game_id, task) = solana_state.pending_participants_fetch.take().unwrap();
    let result = futures_lite::future::block_on(async {
        match task.await {
            Ok(res) => res,
            Err(e) => Err(format!("Task panicked or cancelled: {}", e)),
        }
    });
    match result {
        Ok(Some((white, black))) => {
            info!(
                "[SESSION] Verified on-chain participants for game {}: white={} black={}",
                game_id, white, black
            );
            causal.verified_wallets.insert(game_id, (white, black));
        }
        Ok(None) => {
            debug!(
                "[SESSION] No on-chain participants for game {} — casual game, roster stays trust-first",
                game_id
            );
        }
        Err(e) => {
            warn!(
                "[SESSION] Verified-participants fetch failed for game {}: {} — roster stays trust-first",
                game_id, e
            );
        }
    }
}

fn get_hot_wallet_path() -> Option<PathBuf> {
    #[cfg(target_os = "android")]
    {
        crate::core::paths::internal_data_dir().map(|dir| {
            hot_wallet_filename(&dir, std::env::var("XFCHESS_WALLET_PORT").ok().as_deref())
        })
    }
    #[cfg(not(target_os = "android"))]
    {
        ProjectDirs::from("com", "trilltino", "XFChess").map(|proj_dirs| {
            hot_wallet_filename(
                proj_dirs.config_dir(),
                std::env::var("XFCHESS_WALLET_PORT").ok().as_deref(),
            )
        })
    }
}

fn hot_wallet_filename(config_dir: &std::path::Path, wallet_port: Option<&str>) -> PathBuf {
    match wallet_port.map(str::trim).filter(|p| !p.is_empty()) {
        Some(p) if p != "7454" => config_dir.join(format!("hot_wallet_{p}.json")),
        _ => config_dir.join("hot_wallet.json"),
    }
}

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

pub fn fetch_user_status_async(
    mut solana_wallet: Option<ResMut<crate::multiplayer::solana::addon::SolanaWallet>>,
    time: Res<Time>,
    mut poll_timer: Local<f32>,
    mut rx: Local<Option<crossbeam_channel::Receiver<Option<UserStatus>>>>,
    tokio_runtime: Res<crate::multiplayer::TokioRuntime>,
) {
    // Drain any pending result first
    if let Some(ref receiver) = *rx {
        match receiver.try_recv() {
            Ok(Some(status)) => {
                if let Some(ref mut w) = solana_wallet {
                    info!(
                        "[USER_STATUS] profile={} email={} kyc={} can_wager={}",
                        status.has_profile, status.has_email, status.has_kyc, status.can_wager
                    );
                    w.user_status = Some(status);
                }
                *rx = None;
            }
            Ok(None) => {
                *rx = None;
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {}
            Err(_) => {
                *rx = None;
            }
        }
    }

    // Kick off a new fetch every 30 seconds when connected and idle
    *poll_timer -= time.delta_secs();
    if *poll_timer > 0.0 || rx.is_some() {
        return;
    }
    *poll_timer = 30.0;

    let pubkey = match solana_wallet.as_ref().and_then(|w| w.pubkey) {
        Some(pk) => pk.to_string(),
        None => return,
    };

    let (tx, new_rx) = crossbeam_channel::bounded::<Option<UserStatus>>(1);
    *rx = Some(new_rx);
    tokio_runtime.0.spawn(async move {
        let result = crate::multiplayer::vps_client::get_user_status_async(pubkey)
            .await
            .ok();
        let _ = tx.send(result);
    });
}

pub fn sync_player_profiles(
    mut competitive: ResMut<crate::multiplayer::solana::addon::CompetitiveMatchState>,
    mut profile: ResMut<crate::multiplayer::solana::addon::SolanaProfile>,
    solana_state: Res<SolanaIntegrationState>,
    tokio_runtime: Res<crate::multiplayer::TokioRuntime>,
    mut own_rx: Local<
        Option<
            crossbeam_channel::Receiver<Option<crate::multiplayer::network::vps::PlayerProfile>>,
        >,
    >,
    mut opp_rx: Local<
        Option<
            crossbeam_channel::Receiver<Option<crate::multiplayer::network::vps::PlayerProfile>>,
        >,
    >,
    mut last_game_id: Local<Option<u64>>,
    mut last_opp_pk: Local<Option<Pubkey>>,
) {
    // Trigger fetches
    if competitive.active {
        if competitive.game_id != *last_game_id || competitive.opponent_pubkey != *last_opp_pk {
            *last_game_id = competitive.game_id;
            *last_opp_pk = competitive.opponent_pubkey;

            if let Some(pk) = solana_state.wallet_pubkey {
                let (tx, rx) = crossbeam_channel::bounded(1);
                let pk_str = pk.to_string();
                // Run blocking profile fetches on spawn_blocking, outside the async runtime.
                tokio_runtime.0.spawn(async move {
                    let result = tokio::task::spawn_blocking(move || {
                        crate::multiplayer::network::vps::fetch_player_profile(&pk_str).ok()
                    })
                    .await
                    .unwrap_or(None);
                    let _ = tx.send(result);
                });
                *own_rx = Some(rx);
            }

            if let Some(pk) = competitive.opponent_pubkey {
                let (tx, rx) = crossbeam_channel::bounded(1);
                let pk_str = pk.to_string();
                tokio_runtime.0.spawn(async move {
                    let result = tokio::task::spawn_blocking(move || {
                        crate::multiplayer::network::vps::fetch_player_profile(&pk_str).ok()
                    })
                    .await
                    .unwrap_or(None);
                    let _ = tx.send(result);
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
                info!(
                    "[PROFILES] Updated own profile: {} ({} ELO)",
                    profile.username, profile.elo
                );
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
                info!(
                    "[PROFILES] Updated opponent profile: {} ({} ELO)",
                    competitive.opponent_username, competitive.opponent_elo
                );
            }
            *opp_rx = None;
        }
    }
}

pub fn setup_solana_system(mut commands: Commands) {
    // Placeholder for fetching relayer_pubkey from backend or environment
    let relayer_pubkey = "PlaceholderRelayerPubkey";
    commands.insert_resource(SolanaRpc {
        rpc_url: "https://api.devnet.solana.com".to_string(),
        fee_payer: relayer_pubkey.to_string(),
    });
}

pub fn handle_game_transactions(_game_state: ResMut<GameState>, _solana_rpc: Res<SolanaRpc>) {
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

// -- Item 8: Global session VPS handshake -------------------------------------

#[derive(bevy::prelude::Resource, Debug, Clone)]
pub struct GlobalSessionActive {
    pub session_pubkey: String,
}

#[derive(bevy::prelude::Resource)]
pub struct GlobalSessionCheckPending {
    pub rx: crossbeam_channel::Receiver<Option<String>>,
}

pub fn verify_global_session_on_menu_enter(
    solana_state: Option<bevy::prelude::Res<SolanaIntegrationState>>,
    mut commands: bevy::prelude::Commands,
) {
    let wallet = match solana_state.as_ref().and_then(|s| s.wallet_pubkey) {
        Some(pk) => pk.to_string(),
        None => return,
    };

    // Remove stale resource immediately so the banner shows "checking" state.
    commands.remove_resource::<GlobalSessionActive>();

    let (tx, rx) = crossbeam_channel::bounded::<Option<String>>(1);
    commands.insert_resource(GlobalSessionCheckPending { rx });

    std::thread::spawn(move || {
        use crate::multiplayer::vps_client;
        match vps_client::verify_global_session(&wallet) {
            Ok(Some(session_pubkey)) => {
                info!(
                    "[GLOBAL_SESSION] VPS confirmed active session {} for {}",
                    session_pubkey, wallet
                );
                let _ = tx.send(Some(session_pubkey));
            }
            Ok(None) => {
                info!(
                    "[GLOBAL_SESSION] No active global session on VPS for {}",
                    wallet
                );
                let _ = tx.send(None);
            }
            Err(e) => {
                warn!("[GLOBAL_SESSION] Verify failed for {}: {e}", wallet);
                let _ = tx.send(None);
            }
        }
    });
}

pub fn poll_global_session_result(
    mut commands: bevy::prelude::Commands,
    pending: Option<bevy::prelude::ResMut<GlobalSessionCheckPending>>,
) {
    let Some(pending) = pending else { return };
    match pending.rx.try_recv() {
        Ok(Some(session_pubkey)) => {
            commands.insert_resource(GlobalSessionActive { session_pubkey });
            commands.remove_resource::<GlobalSessionCheckPending>();
        }
        Ok(None) => {
            commands.remove_resource::<GlobalSessionCheckPending>();
        }
        Err(crossbeam_channel::TryRecvError::Empty) => {}
        Err(_) => {
            commands.remove_resource::<GlobalSessionCheckPending>();
        }
    }
}

pub fn authorize_global_session_if_needed(
    mut solana_state: ResMut<SolanaIntegrationState>,
    time: Res<Time>,
    mut rx: Local<Option<crossbeam_channel::Receiver<Result<Keypair, String>>>>,
    mut retry_timer: Local<f32>,
    mut attempted_for: Local<Option<Pubkey>>,
    mut failed_attempts: Local<u32>,
) {
    // Enable global sessions only for embedded wallets; extension wallets still
    // fall back to per-game signing because cluster selection can mismatch.
    if !solana_state.wallet_is_embedded {
        return;
    }

    const MAX_ATTEMPTS: u32 = 3;

    let Some(wallet_pubkey) = solana_state.wallet_pubkey else {
        return;
    };

    if let Some(ref receiver) = *rx {
        match receiver.try_recv() {
            Ok(Ok(kp)) => {
                info!("[GLOBAL_SESSION] Authorized — session {}", kp.pubkey());
                solana_state.global_session_keypair = Some(kp);
                solana_state.global_session_active = true;
                solana_state.global_session_unavailable_reason = None;
                solana_state.global_session_setup_in_progress = false;
                crate::multiplayer::network::vps::emit_client_event(
                    crate::multiplayer::network::vps::ClientEvent::new(
                        "solana_global_session_active",
                    )
                    .wallet(wallet_pubkey)
                    .session_kind("global"),
                );
                *rx = None;
            }
            Ok(Err(e)) => {
                *failed_attempts += 1;
                *rx = None;
                solana_state.global_session_setup_in_progress = false;
                if *failed_attempts >= MAX_ATTEMPTS {
                    warn!(
                        "[GLOBAL_SESSION] Authorization failed {} times ({e}) — giving up for this session, falling back to per-game wallet signing.",
                        *failed_attempts
                    );
                    *retry_timer = f32::INFINITY; // never retry again this run
                    solana_state.global_session_unavailable_reason = Some(e);
                    crate::multiplayer::network::vps::emit_client_event(
                        crate::multiplayer::network::vps::ClientEvent::new(
                            "solana_global_session_unavailable",
                        )
                        .wallet(wallet_pubkey)
                        .session_kind("per_game")
                        .reason(
                            solana_state
                                .global_session_unavailable_reason
                                .clone()
                                .unwrap_or_else(|| "authorization failed".to_string()),
                        ),
                    );
                } else {
                    warn!(
                        "[GLOBAL_SESSION] Authorization failed ({}/{MAX_ATTEMPTS}): {e}",
                        *failed_attempts
                    );
                    *retry_timer = 30.0; // back off before trying again
                }
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {}
            Err(_) => {
                *failed_attempts += 1;
                *rx = None;
                solana_state.global_session_setup_in_progress = false;
                *retry_timer = if *failed_attempts >= MAX_ATTEMPTS {
                    f32::INFINITY
                } else {
                    30.0
                };
                if *failed_attempts >= MAX_ATTEMPTS {
                    solana_state.global_session_unavailable_reason =
                        Some("background task dropped".to_string());
                    crate::multiplayer::network::vps::emit_client_event(
                        crate::multiplayer::network::vps::ClientEvent::new(
                            "solana_global_session_unavailable",
                        )
                        .wallet(wallet_pubkey)
                        .session_kind("per_game")
                        .reason("background task dropped"),
                    );
                }
            }
        }
        return;
    }

    if solana_state.global_session_active {
        return;
    }
    // Don't compete with the (separate, existing) profile-creation gate —
    // wait until that's fully done first.
    if solana_state.profile_status != super::state::ProfileStatus::HasProfileWithUsername {
        return;
    }
    // One attempt per connected wallet per app run, then a cooldown on failure.
    if *attempted_for == Some(wallet_pubkey) {
        *retry_timer -= time.delta_secs();
        if *retry_timer > 0.0 {
            return;
        }
    }

    // Check the wallet covers the session deposit, key funding, and rent/fees
    // before opening an authorization prompt.
    const MIN_BALANCE_FOR_GLOBAL_SESSION_SOL: f64 = 0.115;
    if solana_state.balance < MIN_BALANCE_FOR_GLOBAL_SESSION_SOL {
        solana_state.global_session_unavailable_reason = Some(format!(
            "wallet balance {:.4} SOL is below the {:.3} SOL needed to set up one-time session signing — fund the wallet and it will retry automatically",
            solana_state.balance, MIN_BALANCE_FOR_GLOBAL_SESSION_SOL
        ));
        crate::multiplayer::network::vps::emit_client_event(
            crate::multiplayer::network::vps::ClientEvent::new(
                "solana_global_session_unavailable",
            )
            .wallet(wallet_pubkey)
            .session_kind("per_game")
            .reason("insufficient wallet balance for global session"),
        );
        *attempted_for = Some(wallet_pubkey);
        *retry_timer = 30.0; // re-check periodically in case the wallet gets funded
        return;
    }

    *attempted_for = Some(wallet_pubkey);

    let program_id: Pubkey = crate::solana::instructions::PROGRAM_ID
        .parse()
        .unwrap_or_default();
    let rpc_url = DEVNET_RPC_URL.to_string();
    let (tx, receiver) = crossbeam_channel::bounded(1);
    *rx = Some(receiver);
    solana_state.global_session_setup_in_progress = true;
    std::thread::spawn(move || {
        let _ = tx.send(establish_global_session(
            wallet_pubkey,
            program_id,
            &rpc_url,
        ));
    });
}

fn establish_global_session(
    wallet_pubkey: Pubkey,
    program_id: Pubkey,
    rpc_url: &str,
) -> Result<Keypair, String> {
    use crate::multiplayer::solana::global_session_manager::{
        build_authorize_global_session_ix, build_revoke_global_session_ix,
        build_withdraw_global_session_ix, find_global_session_pda, global_session_account_exists,
        global_session_is_live_onchain, AuthorizeGlobalSessionArgs, GlobalSessionKeyManager,
    };
    use crate::multiplayer::solana::tauri_signer::sign_and_send_via_tauri;

    let mgr = GlobalSessionKeyManager::new(&wallet_pubkey);
    let session_pubkey = mgr.pubkey();
    let (session_pda, _bump) = find_global_session_pda(&program_id, &wallet_pubkey);

    // 0.1 SOL — matches the (currently unused) backend `/global-session/prepare`
    // default; no spending_limit/max_wager cap beyond the deposit itself.
    const DEPOSIT_LAMPORTS: u64 = 100_000_000;
    let authorize_ix = build_authorize_global_session_ix(
        &program_id,
        &wallet_pubkey,
        &session_pda,
        AuthorizeGlobalSessionArgs {
            session_key: session_pubkey,
            duration_secs: None,
            spending_limit: None,
            max_wager: None,
            games: None,
            deposit_lamports: DEPOSIT_LAMPORTS,
        },
    );

    // Fund the fresh client-side session key in the same authorization transaction
    // so it can pay subsequent fees.
    const SESSION_KEY_FUND_LAMPORTS: u64 = 10_000_000;
    let fund_ix = solana_system_interface::instruction::transfer(
        &wallet_pubkey,
        &session_pubkey,
        SESSION_KEY_FUND_LAMPORTS,
    );

    // Revoke a live on-chain delegation with no matching local key before reauthorizing.
    // Retain reactive retry in case this preflight read is stale.
    let needs_revoke_first = global_session_is_live_onchain(rpc_url, &session_pda);
    if needs_revoke_first {
        info!(
            "[GLOBAL_SESSION] On-chain delegation already live (pre-flight check) — revoking before authorizing, no wasted popup"
        );
    }

    // Reclaim expired or exhausted session balances before fresh authorization
    // to avoid accumulating stranded deposits.
    let stale_balance_to_reclaim =
        !needs_revoke_first && global_session_account_exists(rpc_url, &session_pda);
    if stale_balance_to_reclaim {
        info!(
            "[GLOBAL_SESSION] Prior session account exists but is expired/exhausted — reclaiming its balance in the same re-authorize transaction"
        );
    }

    let authorize_result = if needs_revoke_first {
        Err("GlobalSessionAlreadyActive (pre-flight check)".to_string())
    } else if stale_balance_to_reclaim {
        let withdraw_ix =
            build_withdraw_global_session_ix(&program_id, &wallet_pubkey, &session_pda);
        sign_and_send_via_tauri(
            rpc_url,
            wallet_pubkey,
            &[withdraw_ix, authorize_ix.clone(), fund_ix.clone()],
            &[],
            "Setting up one-time quick-sign session",
        )
        .map(|_| ())
    } else {
        sign_and_send_via_tauri(
            rpc_url,
            wallet_pubkey,
            &[authorize_ix.clone(), fund_ix.clone()],
            &[],
            "Setting up one-time quick-sign session",
        )
        .map(|_| ())
    };

    if let Err(e) = authorize_result {
        if e.contains("GlobalSessionAlreadyActive") {
            if !needs_revoke_first {
                info!(
                    "[GLOBAL_SESSION] On-chain delegation already active with no matching local key — revoking before re-authorizing"
                );
            }
            let revoke_ix =
                build_revoke_global_session_ix(&program_id, &wallet_pubkey, &session_pda);
            // Bundle withdrawal with revoke: revoke disables the session without returning lamports.
            let withdraw_ix =
                build_withdraw_global_session_ix(&program_id, &wallet_pubkey, &session_pda);
            sign_and_send_via_tauri(
                rpc_url,
                wallet_pubkey,
                &[revoke_ix, withdraw_ix],
                &[],
                "Revoking stale quick-sign session",
            )
            .map_err(|e2| {
                format!("authorize_global_session: {e} (revoke retry also failed: {e2})")
            })?;
            sign_and_send_via_tauri(
                rpc_url,
                wallet_pubkey,
                &[authorize_ix, fund_ix],
                &[],
                "Setting up one-time quick-sign session",
            )
            .map_err(|e2| format!("authorize_global_session (after revoke): {e2}"))?;
        } else {
            return Err(format!("authorize_global_session: {e}"));
        }
    }

    register_global_session_with_backend(wallet_pubkey, mgr.signer().to_bytes());

    mgr.save(&wallet_pubkey, 30)
        .map_err(|e| format!("save session key: {e}"))?;

    Keypair::try_from(mgr.signer().to_bytes().as_slice())
        .map_err(|e| format!("keypair conversion: {e}"))
}

pub(crate) enum GlobalSessionRegisterOutcome {
    Confirmed,
    ConfirmedMismatch,
    Transient,
}

#[derive(bevy::prelude::Resource)]
pub(crate) struct GlobalSessionRegisterPending {
    rx: crossbeam_channel::Receiver<GlobalSessionRegisterOutcome>,
}

pub fn poll_global_session_register_result(
    mut solana_state: ResMut<SolanaIntegrationState>,
    pending: Option<Res<GlobalSessionRegisterPending>>,
    mut commands: Commands,
) {
    let Some(pending) = pending else { return };
    match pending.rx.try_recv() {
        Ok(GlobalSessionRegisterOutcome::ConfirmedMismatch) => {
            warn!(
                "[GLOBAL_SESSION] Backend confirmed the local session key doesn't match on-chain — clearing it"
            );
            if let Some(wallet) = solana_state.wallet_pubkey {
                crate::multiplayer::network::vps::emit_client_event(
                    crate::multiplayer::network::vps::ClientEvent::new(
                        "solana_global_session_register_failed",
                    )
                    .wallet(wallet)
                    .session_kind("global")
                    .reason("backend confirmed key mismatch"),
                );
            }
            solana_state.global_session_active = false;
            solana_state.global_session_keypair = None;
            commands.remove_resource::<GlobalSessionRegisterPending>();
        }
        Ok(GlobalSessionRegisterOutcome::Confirmed | GlobalSessionRegisterOutcome::Transient) => {
            commands.remove_resource::<GlobalSessionRegisterPending>();
        }
        Err(crossbeam_channel::TryRecvError::Empty) => {}
        Err(_) => {
            commands.remove_resource::<GlobalSessionRegisterPending>();
        }
    }
}

fn register_global_session_with_backend(
    wallet_pubkey: Pubkey,
    keypair_bytes: [u8; 64],
) -> GlobalSessionRegisterOutcome {
    let secret_b58 = bs58::encode(keypair_bytes).into_string();
    let url = format!(
        "{}/api/global-session/register",
        crate::multiplayer::network::vps::vps_base()
    );
    let body = serde_json::json!({
        "wallet_pubkey": wallet_pubkey.to_string(),
        "session_secret_key_b58": secret_b58,
    });
    match reqwest::blocking::Client::new()
        .post(&url)
        .json(&body)
        .send()
    {
        Ok(resp) if resp.status().is_success() => {
            info!("[GLOBAL_SESSION] Registered session key with backend");
            GlobalSessionRegisterOutcome::Confirmed
        }
        Ok(resp)
            if resp.status() == reqwest::StatusCode::FORBIDDEN
                || resp.status() == reqwest::StatusCode::BAD_GATEWAY =>
        {
            warn!(
                "[GLOBAL_SESSION] Backend rejected registration ({}) — key does not match on-chain state",
                resp.status()
            );
            GlobalSessionRegisterOutcome::ConfirmedMismatch
        }
        Ok(resp) => {
            warn!(
                "[GLOBAL_SESSION] Backend registration returned {} — finalize/undelegate/settlement won't work for this wallet until it's retried",
                resp.status()
            );
            GlobalSessionRegisterOutcome::Transient
        }
        Err(e) => {
            warn!("[GLOBAL_SESSION] Backend registration failed: {e}");
            GlobalSessionRegisterOutcome::Transient
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hot_wallet_filename_diverges_for_a_non_default_instance_port() {
        let dir = PathBuf::from("/config");
        let p1 = hot_wallet_filename(&dir, None);
        let p2 = hot_wallet_filename(&dir, Some("7464"));
        assert_eq!(
            p1,
            dir.join("hot_wallet.json"),
            "unset port must keep the pre-fix default path"
        );
        assert_ne!(p1, p2, "a non-default port must get its own file");
    }

    #[test]
    fn hot_wallet_filename_treats_explicit_default_port_same_as_unset() {
        let dir = PathBuf::from("/config");
        let unset = hot_wallet_filename(&dir, None);
        let explicit_default = hot_wallet_filename(&dir, Some("7454"));
        let blank = hot_wallet_filename(&dir, Some("  "));
        assert_eq!(unset, explicit_default);
        assert_eq!(unset, blank);
    }
}
