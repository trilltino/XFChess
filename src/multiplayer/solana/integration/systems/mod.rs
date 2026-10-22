//! Systems behind [`SolanaIntegrationPlugin`](super::plugin::SolanaIntegrationPlugin):
//! wallet balance and rates ([`wallet`]), per-game session keys ([`session_key`]),
//! the sign-once global session ([`global_session`]) and player profiles
//! ([`profiles`]).

use super::state::{BalanceRefreshTimer, SolanaIntegrationState, DEVNET_RPC_URL};
use crate::game::events::GameStartedEvent;
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

mod global_session;
mod profiles;
mod session_key;
mod wallet;
pub use global_session::*;
pub use profiles::*;
pub use session_key::*;
pub use wallet::*;

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
                    match load_or_create_hot_wallet() {
                        Ok(keypair) => {
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
                        Err(error) => {
                            error!("[WALLET] {error}");
                            *retry_secs = 3.0;
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
                                crate::core::net_task::spawn_detached("systems", move || {
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
