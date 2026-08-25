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

/// Solana RPC configuration with relayer fee payer. Only ever constructed by
/// `setup_solana_system` below, which is itself never registered as a system
/// — this resource is never actually inserted into the app. Distinct from
/// (and unrelated to) the differently-unused `solana::rpc::SolanaRpc`.
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
                    // 2. Attempt to parse as real Solana Pubkey
                    match trimmed.parse::<Pubkey>() {
                        Ok(pubkey) => {
                            info!("[WALLET] Phantom wallet connected. Pubkey: {}", pubkey);
                            solana_state.wallet_pubkey = Some(pubkey);
                            solana_state.rpc_client = Some(RpcClient::new_with_commitment(
                                DEVNET_RPC_URL.to_string(),
                                CommitmentConfig::confirmed(),
                            ));
                            if let Some(ref mut w) = solana_wallet {
                                w.pubkey = Some(pubkey);
                            }

                            // Pick up an already-authorized global session from
                            // a previous run, if one's saved on disk — lets
                            // `authorize_global_session_if_needed` skip
                            // straight to "active" instead of re-prompting.
                            // This is optimistic (local file only); the
                            // backend registration below is what actually
                            // confirms it against on-chain state, and can
                            // still flip `global_session_active` back off.
                            solana_state.try_load_global_session(&pubkey);
                            // Re-register with the backend on every connect
                            // (off this thread) — its registry is in-memory
                            // only and forgets everything on restart, so this
                            // is what recovers finalize/undelegate/settlement
                            // for this wallet without needing a fresh popup.
                            // Its result now feeds back into
                            // `global_session_active` (via
                            // `poll_global_session_register_result`) instead
                            // of only being logged — a locally-decryptable
                            // but on-chain-mismatched key (lost/stale file,
                            // wrong instance, different machine) was
                            // previously trusted as "active" until the first
                            // real use failed against the wrong session PDA.
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

                            // Gossip-signing keypair generation lives entirely in
                            // `sync_session_key_to_network` now (gated on
                            // `wallet_pubkey.is_some()`, not on this specific branch
                            // running) — this code path loses the race against
                            // `main_menu.rs`'s faster WalletBridge poll almost every
                            // time in practice, so generating the key here too would
                            // just be dead code most runs.
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

    // Announced port first, full scan only as a fallback — same discovery
    // path as open_wallet_browser()/send_to_tauri_blocking(), not a
    // hand-rolled copy of it (see the wallet-bridge port-discovery audit:
    // this used to have its own inline scan that never got the port-file
    // fast path and would drift out of sync with the canonical one).
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

/// Refreshes the wallet's SOL balance on a timer. The actual RPC call runs on
/// tokio's blocking-thread pool via `spawn_blocking` — `get_balance` is a
/// synchronous network call, and running it straight in a Bevy system (as
/// this used to) blocks that frame's update for the full round-trip, which
/// reads to the player as the whole game hitching every time the balance
/// polls.
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

    // Fetch immediately on a fresh connect instead of waiting for the next
    // periodic tick (up to `BalanceRefreshTimer`'s full interval away, since
    // a freshly-constructed `Timer` doesn't fire on its first tick) — this
    // is the balance `lobby.cached_balance`/wager-eligibility checks read
    // (see `sync_from_solana_state`), so a stale `0.0` here was blocking
    // "join wager" right after connecting even though the separately-tracked
    // `WalletBridgeData.sol_balance` shown in the HUD (refreshed every 5s by
    // `poll_wallet_bridge`) already had the real value.
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

/// Background-fetch SOL/USD rate from CoinGecko and cache it in [`SolanaIntegrationState`].
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

/// Fetches the live SOL/USD rate from the signing backend's cached
/// `/api/rates/all` — the same cache the admin panel, tournament creation,
/// and the in-game wager UI all read, so every USD figure in the app comes
/// from one source instead of each surface hitting its own third-party feed.
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

/// Sync the gossip-signing key into the P2P network state so that all
/// outgoing gossip messages are cryptographically signed.
///
/// Generates the keypair itself (into `solana_state.session_keypair`, kept
/// for display/logging elsewhere) rather than only copying an
/// already-generated one — `initialize_solana_integration` used to be the
/// sole generator, but that only runs on the branch where its *own* Tauri
/// poll resolves the wallet pubkey, which loses the race against
/// `main_menu.rs`'s faster WalletBridge HTTP poll almost every time (the
/// latter's "only set pubkey once" guard then makes
/// `initialize_solana_integration` return before ever reaching the
/// keygen). In practice this meant no gossip-signing key was ever
/// generated, `session_signing_key` stayed `None` for the whole run, and
/// every P2P message went out unsigned — silently dropped by any peer that
/// hasn't opted into `allow-unsigned-p2p`. Generating it here instead,
/// gated only on "wallet connected, no key yet," makes it independent of
/// which of the two wallet-connect code paths actually wins.
pub fn sync_session_key_to_network(
    mut solana_state: ResMut<SolanaIntegrationState>,
    mut network_state: ResMut<OnlineNetworkState>,
) {
    if network_state.session_signing_key.is_some() {
        return;
    }
    if solana_state.wallet_pubkey.is_none() {
        // No wallet connected — casual/local play still needs every gossip
        // message signed (see this function's doc comment above: an
        // unsigned message is silently dropped by any peer without
        // allow-unsigned-p2p, which meant every P2P message — every Move,
        // Clock, Ping — in every non-wallet game was always dropped, with
        // the board never updating for either player). Fall back to the
        // device's persistent node identity key — already loaded at startup
        // (`identity::load_or_create`), already the stable per-device
        // identity used for friends/presence (see social.rs's doc comment:
        // "node ID is the stable identity; Solana pubkey is optional") —
        // rather than waiting forever for a wallet a casual player never
        // connects.
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
        let session_kp = solana_sdk::signature::Keypair::new();
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
        // `SessionInfo` (which populates `opponent_pubkey`, required before
        // `bridge.rs` can finalize the game on-chain) previously went out
        // over Iroh gossip only. Moves and resignation already learned this
        // lesson (see `online_game_session.rs`'s "dual transport" comment) —
        // gossip alone silently drops this message whenever the direct P2P
        // link hasn't established (a real, reproduced failure: "[P2P]
        // Connection timed out after 12s"), leaving both sides stuck logging
        // "Opponent pubkey unavailable" forever and the game never settling.
        let node_b58 = network_state
            .node_id
            .as_ref()
            .map(|id| bs58::encode(id.as_bytes()).into_string());
        // The pubkey that actually signs this connection's outgoing gossip
        // envelopes (see `sync_session_key_to_network`) — distinct from
        // `session_pubkey` below. The per-game participant roster
        // (`multiplayer::systems`'s causal-broadcast check) is keyed off
        // *this* value, since that's what a received move's verified signer
        // is compared against, not the VPS session-delegation key.
        //
        // `session_signing_key` stores the 32-byte Ed25519 *seed*, not the
        // public key — `SignedNetworkMessage::sign`/`verify` derive the real
        // verifying key from it via `SigningKey::from_bytes(seed)
        // .verifying_key()`. This MUST use the same derivation: an earlier
        // version of this code passed the raw seed bytes straight into
        // `Pubkey::new_from_array`, which just reinterprets 32 arbitrary
        // bytes as if they were already a public key — not a valid Ed25519
        // public-key derivation, and never equal to what `bind_identity`
        // actually verifies and sets as `signer_pubkey`. That bug meant the
        // roster was still populated with a value no real move's verified
        // signer could ever match, silently reproducing the exact
        // "non-participant signer" rejection the `signing_pubkey` field was
        // added to fix in the first place.
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

                            // Dual transport, same reasoning as moves/resign:
                            // gossip alone can silently drop this before the
                            // P2P link establishes — a real, previously
                            // reproduced bug ("Opponent pubkey unavailable"
                            // forever, game never settles). SessionInfo is
                            // Both players post one of these into the same
                            // shared moves stream, so the parent comes from
                            // the tracked stream head rather than being
                            // assumed to be genesis.
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

/// Spawns the on-chain participants fetch (`docs/plans/networking-hardening-plan.md`'s
/// Phase C) once per game start — the client-side counterpart to Phase B's
/// backend `GameParticipantsCache`. Skips if a fetch for this exact game is
/// already pending or has already completed (`verified_wallets` already has
/// an entry) — `GameStartedEvent` can fire more than once for tournament
/// games and a retry loop shouldn't re-spawn on every one.
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

        // `fetch_verified_participants` builds and uses a *blocking*
        // reqwest client internally — calling it from inside a plain
        // `.spawn(async move { ... })` task runs it directly on the async
        // runtime's own worker thread, and reqwest::blocking constructing
        // its own nested runtime in that context panics ("Cannot drop a
        // runtime in a context where blocking is not allowed"). Must go
        // through `spawn_blocking` instead, same as `update_wallet_balance`'s
        // RPC call — moves it to a dedicated blocking-pool thread.
        let handle = tokio_runtime.0.spawn_blocking(move || {
            crate::multiplayer::vps_client::fetch_verified_participants(game_id)
        });
        solana_state.pending_participants_fetch = Some((game_id, handle));
    }
}

/// Polls the fetch spawned by [`spawn_verified_participants_fetch`] and, on
/// a successful on-chain result, seeds `CausalChainState::verified_wallets`
/// — see that field's doc comment for how it's then used to validate
/// `SessionInfo` claims. `Ok(None)` (casual game, no on-chain `Game`
/// account) and any error are both silently dropped: the roster simply
/// keeps its original trust-first bootstrap for that game, which is the
/// correct behavior for casual play and a safe fallback on a transient
/// fetch failure for wagered play (Phase B still independently protects the
/// authoritative Braid log server-side regardless of what this client-local
/// roster admits).
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

/// Resolves the storage path for the local hot wallet.
///
/// Namespaced by `XFCHESS_WALLET_PORT` (same env var `tauri_signer` uses for
/// the wallet-bridge port) whenever it's set to something other than the
/// default 7454 — otherwise two instances on one machine (e.g. `just dev2`'s
/// P1/P2) both read/write the exact same `hot_wallet.json` and end up being
/// the *same* on-chain wallet, so logging out and back in on one window
/// resurfaces whichever identity the other window last saved. Mirrors
/// `network::identity::key_path`'s `XFCHESS_NODE_KEY_PATH` override for the
/// same class of bug on the P2P node key.
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

/// Appends an `_<port>` suffix to `hot_wallet.json` when `wallet_port` is set
/// to something other than the default `7454`. Takes the port as a plain
/// argument rather than reading the env var itself so the namespacing logic
/// can be unit-tested without mutating process-global state.
fn hot_wallet_filename(config_dir: &std::path::Path, wallet_port: Option<&str>) -> PathBuf {
    match wallet_port.map(str::trim).filter(|p| !p.is_empty()) {
        Some(p) if p != "7454" => config_dir.join(format!("hot_wallet_{p}.json")),
        _ => config_dir.join("hot_wallet.json"),
    }
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

/// Fetches user verification status from VPS and caches it in SolanaWallet.
/// Triggers every 30 seconds; result arrives asynchronously via a channel
/// and is applied on a subsequent frame.
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

/// Syncs own and opponent profiles from VPS when a competitive match starts
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

            // Fetch own
            if let Some(pk) = solana_state.wallet_pubkey {
                let (tx, rx) = crossbeam_channel::bounded(1);
                let pk_str = pk.to_string();
                // fetch_player_profile is a blocking reqwest call — it must run via
                // spawn_blocking, not directly inside this async task. reqwest::blocking
                // builds (and later drops) its own nested Tokio runtime internally, which
                // panics ("cannot drop a runtime in a context where blocking is not
                // allowed") when called from a task already driven by this outer runtime.
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

            // Fetch opponent
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

/// Not registered as a system anywhere — `SolanaIntegrationPlugin::build`
/// does not call this, so `SolanaRpc` is never actually inserted as a
/// resource in the running app.
pub fn setup_solana_system(mut commands: Commands) {
    // Placeholder for fetching relayer_pubkey from backend or environment
    let relayer_pubkey = "PlaceholderRelayerPubkey";
    commands.insert_resource(SolanaRpc {
        rpc_url: "https://api.devnet.solana.com".to_string(),
        fee_payer: relayer_pubkey.to_string(),
    });
}

/// Not registered as a system anywhere; body is an unimplemented placeholder.
pub fn handle_game_transactions(_game_state: ResMut<GameState>, _solana_rpc: Res<SolanaRpc>) {
    // Use solana_rpc.fee_payer for transactions
    // Placeholder for transaction logic
}

/// Not registered as a system anywhere; body is an unimplemented placeholder.
pub fn handle_tournament_transactions(
    _tournament_state: ResMut<TournamentClientState>,
    _solana_rpc: Res<SolanaRpc>,
) {
    // Use solana_rpc.fee_payer for tournament transactions
    // Placeholder for transaction logic
}

// -- Item 8: Global session VPS handshake -------------------------------------

/// Resource present when the VPS has confirmed an active global session for the local wallet.
#[derive(bevy::prelude::Resource, Debug, Clone)]
pub struct GlobalSessionActive {
    pub session_pubkey: String,
}

/// Holds the background receiver for the global session verification result.
#[derive(bevy::prelude::Resource)]
pub struct GlobalSessionCheckPending {
    pub rx: crossbeam_channel::Receiver<Option<String>>,
}

/// System that runs once on `OnEnter(MainMenu)` to verify the VPS holds an
/// active global session for this wallet. Spawns a background thread and stores
/// a receiver so `poll_global_session_result` can pick up the answer next frame.
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

/// Polls the global session check receiver and inserts/removes `GlobalSessionActive`.
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

/// Once a wallet is connected and has an on-chain profile, automatically
/// authorize a global session — one Phantom popup, ever — if none is active
/// yet. After this succeeds, `lobby.rs`'s create/join (and, once wired,
/// delegation) can sign locally with the persisted session keypair instead
/// of round-tripping through the Tauri wallet bridge every game.
///
/// Mirrors `profile_check.rs`'s background-task + backoff-timer shape: a
/// `Local` receiver drains a background thread's result, and a cooldown
/// timer stops a failed attempt from retrying every frame.
///
/// Capped at `MAX_ATTEMPTS`: this is purely an optimization (it lets later
/// games sign locally instead of round-tripping through the wallet popup),
/// so a wallet that can't complete it — insufficient balance, a stuck
/// blockhash, whatever — must not be re-prompted with a fresh Solflare
/// popup forever. Giving up just means every game falls back to the
/// per-game Tauri wallet-bridge signing that already existed before this
/// optimization.
pub fn authorize_global_session_if_needed(
    mut solana_state: ResMut<SolanaIntegrationState>,
    time: Res<Time>,
    mut rx: Local<Option<crossbeam_channel::Receiver<Result<Keypair, String>>>>,
    mut retry_timer: Local<f32>,
    mut attempted_for: Local<Option<Pubkey>>,
    mut failed_attempts: Local<u32>,
) {
    // Disabled 2026-08-11. Status of the three original blockers, re-checked
    // 2026-08-23 while wiring up social-login embedded wallets:
    //
    //  1. FIXED (on-chain). "Revoke never reclaims the old deposit" — there is
    //     now a `withdraw_global_session` instruction
    //     (`account_ix::global_session_ix::handler_withdraw_global_session`,
    //     exposed in lib.rs) that returns the unspent vault balance to the
    //     player while keeping the account rent-exempt.
    //
    //  2. NOT APPLICABLE to embedded wallets. The Solflare "network mismatch:
    //     current network devnet, but this transaction is for mainnet"
    //     rejection is an artifact of an extension having its own
    //     user-selected cluster that `ensureDevnet` has to nudge. A Privy
    //     embedded wallet has no such setting, so this failure mode cannot
    //     occur on that path. Still unresolved for extension wallets.
    //
    //  3. FIXED IN SOURCE, NOT YET DEPLOYED. `global_create_game` hitting
    //     `InsufficientFunds` (6060) because only the soft
    //     `spending_limit`/`max_wager` caps were checked, never the vault's
    //     real balance. `game_ix::global_create` and `game_ix::global_join`
    //     now both verify the vault covers rent + wager and stays rent-exempt,
    //     returning `GlobalSessionVaultUnderfunded` with an actionable message
    //     instead. **That guard only takes effect once the program is
    //     redeployed to devnet.**
    //
    // (3) was deployed to devnet on 2026-08-23 (slot 486887670, sig
    // 25ZoXVbwUx9Z4HB13GLDpqEKsMXmL7oE2vucgfYTqq6BahLJgXs7cH1xzVcxmFJAuxpBcH9xaGXqo67GV1AcDAzi),
    // and `global_create_game_tests` proves the guard both rejects an
    // underfunded vault and still accepts an adequately funded one.
    //
    // So the blanket disable is lifted — but only for embedded wallets, since
    // (2) is still open for extensions. Everyone else keeps the proven per-game
    // Tauri wallet-bridge signing path (`cached_global_session_keypair_bytes`
    // stays `None` — see `sync_from_solana_state` in lobby.rs), which is the
    // documented fallback in docs/plans/social-login-embedded-wallet-plan.md
    // §8.4: play works fully without this, it just costs a popup per
    // transaction.
    //
    // To extend this to extension wallets, root-cause (2) first — reproducing
    // it needs Solflare with a non-devnet cluster selected.
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

    // Mirrors `establish_global_session`'s DEPOSIT_LAMPORTS (0.1 SOL) +
    // SESSION_KEY_FUND_LAMPORTS (0.01 SOL), plus a small fee/rent buffer.
    // A wallet below this can never complete the authorize+fund transaction
    // — Phantom will show a real "Confirm Transaction" popup that fails
    // simulation with "funds may be lost if submitted," and the client sits
    // blocked on that popup for up to `SIGN_TIMEOUT_SECS` (60s) per attempt
    // with nothing telling the player why. Check first and skip straight to
    // the labeled unavailable-reason instead of opening a doomed popup.
    const MIN_BALANCE_FOR_GLOBAL_SESSION_SOL: f64 = 0.115;
    if solana_state.balance < MIN_BALANCE_FOR_GLOBAL_SESSION_SOL {
        solana_state.global_session_unavailable_reason = Some(format!(
            "wallet balance {:.4} SOL is below the {:.3} SOL needed to set up one-time session signing — fund the wallet and it will retry automatically",
            solana_state.balance, MIN_BALANCE_FOR_GLOBAL_SESSION_SOL
        ));
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

/// One-time (per wallet) setup: generate a session keypair locally, build the
/// `authorize_global_session` instruction, sign + submit it via the existing
/// Tauri wallet bridge (the one popup this ever costs), then persist the
/// keypair encrypted to disk so `try_load_global_session` finds it on every
/// future launch.
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

    // The session keypair is freshly generated client-side and holds zero
    // SOL — with no backend involved to fund it (unlike the per-game flow's
    // `activate_session`, which funds its session key from the fee-payer
    // pool), it can't even pay its own transaction fee. Bundle a direct
    // transfer into this same, already-necessary signature: 0.01 SOL covers
    // ~2000 future tx fees, same margin `activate_session` uses.
    const SESSION_KEY_FUND_LAMPORTS: u64 = 10_000_000;
    let fund_ix = solana_system_interface::instruction::transfer(
        &wallet_pubkey,
        &session_pubkey,
        SESSION_KEY_FUND_LAMPORTS,
    );

    // A `GlobalSessionDelegation` that's still enabled on-chain but has no
    // matching local key (the local file was lost, overwritten by another
    // instance pre-dating the storage-path fix, or the wallet authorized
    // from a different machine) makes the program reject *every* plain
    // re-authorize attempt with `GlobalSessionAlreadyActive`, identically,
    // forever. Check for that case up front so the common repro doesn't pay
    // for a doomed first popup before revoking — see
    // `global_session_is_live_onchain`'s doc comment for exactly what this
    // mirrors. The reactive fallback below (retry on a real
    // `GlobalSessionAlreadyActive` error) stays as defense-in-depth in case
    // this pre-flight read is stale or races a concurrent authorize.
    let needs_revoke_first = global_session_is_live_onchain(rpc_url, &session_pda);
    if needs_revoke_first {
        info!(
            "[GLOBAL_SESSION] On-chain delegation already live (pre-flight check) — revoking before authorizing, no wasted popup"
        );
    }

    // Fund-leak fix: an account that exists but is no longer "live" (past
    // its 30-day duration, or `games_remaining` hit 0) is NOT caught by
    // `needs_revoke_first` above — the on-chain re-auth guard happily lets a
    // plain re-authorize through in that state. Before this check, that meant
    // every natural re-auth silently deposited a fresh amount on top of
    // whatever balance the expired/exhausted session left behind, with no
    // withdraw ever firing — an unbounded, compounding stranded balance.
    // Reclaim it in the same signature as the fresh authorize whenever the
    // account exists at all and doesn't already need a revoke first (the
    // `needs_revoke_first` branch below handles reclaiming for the live case).
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
            // Bundled with the revoke (one signature, no extra popup): a
            // stale session's leftover deposit is otherwise permanently
            // stranded — `revoke_global_session` only flips `enabled`, it
            // never returns lamports, and the fresh authorize below deposits
            // a brand new amount on top instead of reusing it. Confirmed
            // live 2026-08-11: a wallet whose local key file went missing
            // once ended up with a `GlobalSessionDelegation` vault at 2x the
            // expected balance from exactly this path.
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

/// Result of asking the backend to verify+register an already-authorized
/// global session key. Distinguishes a *confirmed* on-chain mismatch (the
/// backend read the real `GlobalSessionDelegation` PDA and the key genuinely
/// doesn't match — the local file is stale/wrong-wallet/wrong-instance, and
/// must not keep being trusted) from a merely transient failure (backend
/// unreachable, momentary error), which says nothing about whether the key
/// is actually still valid on-chain and must not cause a false "not active".
pub(crate) enum GlobalSessionRegisterOutcome {
    Confirmed,
    ConfirmedMismatch,
    Transient,
}

/// Holds the background `register_global_session_with_backend` receiver so
/// `poll_global_session_register_result` can apply its outcome next frame.
#[derive(bevy::prelude::Resource)]
pub(crate) struct GlobalSessionRegisterPending {
    rx: crossbeam_channel::Receiver<GlobalSessionRegisterOutcome>,
}

/// Drains the background registration result and applies it. The critical
/// case is `ConfirmedMismatch`: previously this result was only logged, so a
/// locally-decryptable but on-chain-mismatched session key (lost/stale file,
/// wrong `dev2` instance, different machine) stayed "active" in
/// `SolanaIntegrationState` until the first real transaction using it failed
/// on-chain against the wrong session PDA — by then the player had already
/// clicked Create/Join expecting the zero-popup path.
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

/// Best-effort: hand the backend a copy of an already-authorized global
/// session key, so it can act on this wallet's behalf for
/// finalize/undelegate/settlement (Track 1 of
/// docs/plans/global-session-flow-fix-plan.md) — create/join/delegate don't
/// need this, the client signs those itself. Idempotent (the backend just
/// re-verifies against on-chain state and overwrites its in-memory entry),
/// so it's safe to call on every wallet connect, not just the first-ever
/// authorization — that repetition matters because the backend's registry
/// is in-memory only and forgets everything on restart.
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

    /// Guards the `dev2` P1/P2 instance-isolation fix: two windows on one
    /// machine must not silently share `hot_wallet.json` (that was the
    /// "logout logs me in as the other instance" bug — this file has no
    /// wallet-pubkey mismatch check the way the session-key managers do, so
    /// a shared path here means both windows are silently the *same* wallet).
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
