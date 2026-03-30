use bevy::prelude::*;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;

use crate::game::events::GameStartedEvent;
use crate::multiplayer::{BraidNetworkState, NetworkMessage};

use super::state::{BalanceRefreshTimer, SolanaIntegrationState, DEVNET_RPC_URL};

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
                match pubkey_str.parse::<Pubkey>() {
                    Ok(pubkey) => {
                        info!("[WALLET] Phantom wallet connected. Pubkey: {}", pubkey);
                        solana_state.wallet_pubkey = Some(pubkey);
                        solana_state.rpc_client =
                            Some(RpcClient::new(DEVNET_RPC_URL.to_string()));
                        if let Some(ref mut w) = solana_wallet {
                            w.pubkey = Some(pubkey);
                        }
                    }
                    Err(e) => warn!("[WALLET] Invalid pubkey from Tauri: {}", e),
                }
            }
            Ok(None) => {
                *rx = None;
                *retry_secs = 2.0; 
            }
            Err(crossbeam_channel::TryRecvError::Disconnected) => {
                *rx = None;
                *retry_secs = 1.0;
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {} 
        }
        return;
    }

    if *retry_secs > 0.0 {
        *retry_secs -= time.delta_secs();
        return;
    }

    let (tx, receiver) = crossbeam_channel::bounded(1);
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
    match rpc_client.get_balance(pubkey) {
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
    if let Some(mut task) = solana_state.pending_task.take() {
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
                use crate::multiplayer::rollup::vps_client;

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
