#![allow(dead_code)]
//! Profile check system - Detects if wallet needs profile creation
//!
//! When a wallet connects, checks if the player has a profile with username.
//! If not, redirects to ProfileCreation menu state.

use bevy::prelude::*;
use solana_client::rpc_client::RpcClient;

use crate::multiplayer::solana::integration::state::{ProfileStatus, SolanaIntegrationState, DEVNET_RPC_URL};
use crate::multiplayer::TokioRuntime;
use anchor_lang::AccountDeserialize;
use xfchess_game::state::PlayerProfile;

/// System to check profile status when wallet connects
pub fn check_profile_on_connect(
    mut solana_state: ResMut<SolanaIntegrationState>,
    tokio_runtime: Res<TokioRuntime>,
    mut last_wallet: Local<Option<String>>,
) {
    // Get current wallet pubkey as string for comparison
    let current_wallet = solana_state.wallet_pubkey.map(|pk| pk.to_string());

    // Skip if no wallet connected or same wallet as before
    if current_wallet.is_none() || current_wallet == *last_wallet {
        return;
    }

    let Some(wallet_pubkey) = solana_state.wallet_pubkey else {
        warn!("[PROFILE] No wallet pubkey available for check");
        return;
    };
    *last_wallet = Some(wallet_pubkey.to_string());

    // Skip if already checking
    if solana_state.checking_profile || solana_state.pending_profile_check.is_some() {
        return;
    }

    // Mark as checking
    solana_state.checking_profile = true;
    solana_state.profile_status = ProfileStatus::Unknown;

    info!("[PROFILE] Spawning async profile check for wallet: {}", wallet_pubkey);

    let rpc_url = DEVNET_RPC_URL.to_string();
    let profile_pda = solana_state.get_profile_pda(&wallet_pubkey);

    let handle = tokio_runtime.0.spawn(async move {
        let rpc = RpcClient::new(rpc_url);

        match rpc.get_account(&profile_pda) {
            Ok(account) => {
                let mut data: &[u8] = &account.data;
                match PlayerProfile::try_deserialize(&mut data) {
                    Ok(profile) => {
                        if profile.username_set {
                            Ok(ProfileStatus::HasProfileWithUsername)
                        } else {
                            Ok(ProfileStatus::HasProfileNoUsername)
                        }
                    }
                    Err(e) => {
                        // SPECIFIC FIX: If we get AccountDidNotDeserialize (3003), it means the discriminator is wrong/legacy.
                        // We treat this as "NoProfile" so the game can re-initialize it correctly in CreateGame/JoinGame.
                        let e_str = e.to_string();
                        if e_str.contains("3003") || e_str.contains("AccountDidNotDeserialize") {
                            warn!("[PROFILE] Legacy profile detected (3003). Treating as NoProfile for re-init.");
                            Ok(ProfileStatus::NoProfile)
                        } else {
                            Err(format!("Failed to deserialize profile: {}", e))
                        }
                    }
                }
            }
            Err(_) => {
                // Account not found or network error
                Ok(ProfileStatus::NoProfile)
            }
        }
    });

    solana_state.pending_profile_check = Some(handle);
}

/// System to handle the results of the async profile check
pub fn handle_profile_check_tasks(
    mut solana_state: ResMut<SolanaIntegrationState>,
) {
    if let Some(task) = solana_state.pending_profile_check.take() {
        if task.is_finished() {
            // Task is done, get the result
            let result = futures_lite::future::block_on(async {
                match task.await {
                    Ok(res) => res,
                    Err(e) => Err(format!("Task panicked or cancelled: {}", e)),
                }
            });

            solana_state.checking_profile = false;

            match result {
                Ok(status) => {
                    info!("[PROFILE] Async check complete: {:?}", status);
                    solana_state.profile_status = status;

                    // If profile is missing or incomplete, just log it for now
                    // We disable mandatory redirection to let the user test on-chain matches 
                    // even if the profile update isn't deployed yet.
                    if status == ProfileStatus::NoProfile || status == ProfileStatus::HasProfileNoUsername {
                        info!("[PROFILE] Profile is missing or incomplete, but skipping redirection for testing");
                        // menu_state.set(MenuState::ProfileCreation);
                    }
                }
                Err(e) => {
                    error!("[PROFILE] Async check failed: {}", e);
                    // Fallback: indicate profile issue but don't block
                    solana_state.profile_status = ProfileStatus::NoProfile;
                    // menu_state.set(MenuState::ProfileCreation);
                }
            }
        } else {
            // Task still running, put it back
            solana_state.pending_profile_check = Some(task);
        }
    }
}

/// System to auto-initialize profile when entering ProfileCreation without one
pub fn auto_init_profile(
    solana_state: ResMut<SolanaIntegrationState>,
) {
    // Only if we have a wallet and no profile
    if solana_state.wallet_pubkey.is_none() {
        return;
    }
    
    if solana_state.profile_status != ProfileStatus::NoProfile {
        return;
    }
    
    // In a real implementation, this would submit init_profile transaction
    // For now, just update status to indicate profile should be initialized
    info!("[PROFILE] Auto-init profile called - would submit init_profile tx");
    
    // After init_profile succeeds, the status would be updated to HasProfileNoUsername
    // and user can proceed to set username
}
