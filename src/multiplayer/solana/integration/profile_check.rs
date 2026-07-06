//! Profile check system - Detects if wallet needs profile creation
//!
//! When a wallet connects, checks if the player has a profile with username.
//! If not, redirects to ProfileCreation menu state.

use bevy::prelude::*;
use solana_client::rpc_client::RpcClient;

use crate::multiplayer::solana::integration::state::{
    ProfileStatus, SolanaIntegrationState, DEVNET_RPC_URL,
};
use crate::multiplayer::TokioRuntime;
use anchor_lang::AccountDeserialize;
use xfchess_game::state::PlayerProfile;

/// System to check profile status when wallet connects
pub fn check_profile_on_connect(
    mut solana_state: ResMut<SolanaIntegrationState>,
    tokio_runtime: Res<TokioRuntime>,
    mut last_wallet: Local<Option<String>>,
) {
    let current_wallet = solana_state.wallet_pubkey.map(|pk| pk.to_string());
    if current_wallet.is_none() || current_wallet == *last_wallet {
        return;
    }

    let Some(wallet_pubkey) = solana_state.wallet_pubkey else {
        return;
    };
    *last_wallet = Some(wallet_pubkey.to_string());

    if solana_state.checking_profile || solana_state.pending_profile_check.is_some() {
        return;
    }

    solana_state.checking_profile = true;
    solana_state.profile_status = ProfileStatus::Unknown;

    info!(
        "[PROFILE] Spawning async profile check for wallet: {}",
        wallet_pubkey
    );

    let rpc_url = DEVNET_RPC_URL.to_string();
    let profile_pda = solana_state.get_profile_pda(&wallet_pubkey);

    let handle = tokio_runtime.0.spawn(async move {
        let rpc = RpcClient::new(rpc_url);
        match rpc.get_account(&profile_pda) {
            Ok(account) => {
                let mut data: &[u8] = &account.data;
                match PlayerProfile::try_deserialize(&mut data) {
                    Ok(profile) => {
                        let status = if profile.username_set {
                            ProfileStatus::HasProfileWithUsername
                        } else {
                            ProfileStatus::HasProfileNoUsername
                        };
                        let elo = Some(profile.elo_rating as u16);
                        let display_name = if profile.username_set && !profile.username.is_empty() {
                            Some(profile.username.clone())
                        } else {
                            None
                        };
                        Ok((status, elo, display_name))
                    }
                    Err(e) => {
                        let e_str = e.to_string();
                        if e_str.contains("3003") || e_str.contains("AccountDidNotDeserialize") {
                            warn!(
                                "[PROFILE] Legacy profile detected (3003). Treating as NoProfile."
                            );
                            Ok((ProfileStatus::NoProfile, None, None))
                        } else {
                            Err(format!("Failed to deserialize profile: {}", e))
                        }
                    }
                }
            }
            Err(_) => Ok((ProfileStatus::NoProfile, None, None)),
        }
    });

    solana_state.pending_profile_check = Some(handle);
}

/// System to handle the results of the async profile check.
/// Populates cached_elo and cached_display_name; redirects to ProfileCreation when needed.
pub fn handle_profile_check_tasks(mut solana_state: ResMut<SolanaIntegrationState>) {
    if let Some(task) = solana_state.pending_profile_check.take() {
        if task.is_finished() {
            let result = futures_lite::future::block_on(async {
                match task.await {
                    Ok(res) => res,
                    Err(e) => Err(format!("Task panicked or cancelled: {}", e)),
                }
            });

            solana_state.checking_profile = false;

            match result {
                Ok((status, elo, display_name)) => {
                    info!("[PROFILE] Async check complete: {:?}", status);
                    solana_state.profile_status = status;

                    if let Some(e) = elo {
                        solana_state.cached_elo = e;
                    }
                    if display_name.is_some() {
                        solana_state.cached_display_name = display_name;
                    }

                    if status == ProfileStatus::NoProfile
                        || status == ProfileStatus::HasProfileNoUsername
                    {
                        // Profile creation is handled in the Tauri popup — open it via bridge.
                        info!("[PROFILE] Profile incomplete — opening Tauri profile step");
                        std::thread::spawn(|| {
                            let _ = reqwest::blocking::Client::new()
                                .post("http://127.0.0.1:7454/api/open-profile-step")
                                .send();
                        });
                    }
                }
                Err(e) => {
                    error!("[PROFILE] Async check failed: {}", e);
                    solana_state.profile_status = ProfileStatus::NoProfile;
                }
            }
        } else {
            solana_state.pending_profile_check = Some(task);
        }
    }
}

/// System to auto-initialize profile when entering ProfileCreation without one
pub fn auto_init_profile(solana_state: ResMut<SolanaIntegrationState>) {
    if solana_state.wallet_pubkey.is_none() {
        return;
    }
    if solana_state.profile_status != ProfileStatus::NoProfile {
        return;
    }
    info!("[PROFILE] Auto-init profile called — would submit init_profile tx");
}
