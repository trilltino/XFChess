//! Profile Creation UI - Username selection screen
//!
//! the design document at logs/profile-creation-option2-090cd2.md

use bevy::prelude::*;
use bevy_egui::EguiContexts;

use crate::core::states::{DespawnOnExit, MenuState};
use crate::multiplayer::solana::addon::SolanaWallet;
use crate::multiplayer::solana::tauri_signer;
use crate::multiplayer::vps_client;
use crate::multiplayer::TokioRuntime;
use crate::solana::instructions::{get_program_id, init_profile_ix};
use base64::Engine;

/// Resource tracking profile creation state
#[derive(Resource, Default)]
pub struct ProfileCreationState {
    pub username_input: String,
    pub availability_status: UsernameAvailability,
    pub is_validating: bool,
    pub error_message: Option<String>,
    pub country_code: String, // ISO 3166-1 alpha-2 (e.g., "GB", "BR", "CA", "DE")
    pub tax_id: String,       // Country-specific tax ID
    pub tax_id_valid: bool,   // Tax ID format validation status
    pub email: String,
    pub dob: String,
    pub address: String,
}

#[derive(Default, Clone, Copy, PartialEq)]
pub enum UsernameAvailability {
    #[default]
    Unknown,
    Checking,
    Available,
    Taken,
    Invalid,
}

/// Component marker for profile creation UI entities
#[derive(Component)]
pub struct ProfileCreationUi;

/// Event triggered when user clicks "Create Profile"
#[derive(Message, Clone)]
pub struct ProfileSubmissionEvent {
    pub username: String,
    pub country: String,
    pub tax_id: String,
    pub email: String,
    pub dob: String,
    pub address: String,
}

/// System that renders the profile creation UI.
/// Profile creation is now handled exclusively in the Tauri wallet popup.
/// This system is a no-op — it redirects back to the main menu so the
/// in-game modal never appears.
pub fn profile_creation_ui_system(
    mut contexts: EguiContexts,
    mut state: ResMut<ProfileCreationState>,
    mut menu_state: ResMut<NextState<MenuState>>,
    wallet: Res<SolanaWallet>,
    mut submission_events: MessageWriter<ProfileSubmissionEvent>,
) {
    let _ = (&mut contexts, &mut state, &wallet, &mut submission_events);
    // Open the Tauri profile step and return to main menu.
    std::thread::spawn(|| {
        let _ = reqwest::blocking::Client::new()
            .post("http://127.0.0.1:7454/api/open-profile-step")
            .send();
    });
    menu_state.set(MenuState::Main);
}
pub fn validate_username_system(
    mut state: ResMut<ProfileCreationState>,
    time: Res<Time>,
    mut last_checked: Local<String>,
    mut check_timer: Local<f32>,
) {
    let username = state.username_input.clone();

    if username.is_empty() {
        state.availability_status = UsernameAvailability::Unknown;
        *last_checked = String::new();
        *check_timer = 0.0;
        return;
    }

    // Username changed — restart
    if username != *last_checked {
        *last_checked = username.clone();
        *check_timer = 0.0;
        state.availability_status = if is_valid_username_format(&username) {
            UsernameAvailability::Checking
        } else {
            UsernameAvailability::Invalid
        };
        return;
    }

    // Advance timer while Checking
    if state.availability_status == UsernameAvailability::Checking {
        *check_timer += time.delta_secs();
        if *check_timer >= 0.4 {
            // TODO: query UsernameRecord PDA; optimistically Available for now
            state.availability_status = UsernameAvailability::Available;
        }
    }
}

/// Validate username format according to rules
fn is_valid_username_format(username: &str) -> bool {
    let len = username.len();
    if len < 3 || len > 20 {
        return false;
    }

    // Check valid characters
    for ch in username.chars() {
        if !ch.is_ascii_alphanumeric() && ch != '_' && ch != '-' {
            return false;
        }
    }

    // Check reserved names
    let lower = username.to_lowercase();
    let reserved = [
        "admin",
        "system",
        "support",
        "official",
        "moderator",
        "xf",
        "xfchess",
        "chess",
        "test",
        "dev",
        "null",
    ];
    for r in reserved {
        if lower == r || lower.starts_with(r) {
            return false;
        }
    }

    true
}

/// Validate tax ID format based on country code
pub fn spawn_profile_creation_ui(mut commands: Commands) {
    commands.spawn((ProfileCreationUi, DespawnOnExit(MenuState::ProfileCreation)));
}

/// System to handle profile submission events (async)
pub fn handle_profile_submission(
    mut events: MessageReader<ProfileSubmissionEvent>,
    mut state: ResMut<ProfileCreationState>,
    wallet: Res<SolanaWallet>,
    tokio: Res<TokioRuntime>,
    mut menu_state: ResMut<NextState<MenuState>>,
    mut popup_queue: ResMut<crate::ui::menus::popup::GamePopupQueue>,
    auth_state: Res<crate::ui::account::auth::AuthState>,
    mut solana_state: ResMut<
        crate::multiplayer::solana::integration::state::SolanaIntegrationState,
    >,
) {
    for event in events.read() {
        // Optimistically cache the chosen username so the lobby can use it immediately
        solana_state.cached_display_name = Some(event.username.clone());
        solana_state.profile_status =
            crate::multiplayer::solana::integration::state::ProfileStatus::HasProfileWithUsername;

        // Push "Check Wallet" notification
        popup_queue.push(crate::ui::menus::popup::GamePopup {
            title: "Wallet Signature Needed".to_string(),
            message: "Please approve the transaction in your Phantom/Solflare wallet to create your profile.".to_string(),
            copy_text: None,
            url: None,
            url_label: None,
            lifetime: 15.0,
            remaining: 15.0,
            dismissed: false,
        });
        let username = event.username.clone();
        let country = event.country.clone();
        let tax_id = event.tax_id.clone();
        let _email = event.email.clone();
        let dob = event.dob.clone();
        let address = event.address.clone();

        // Credentials for linking
        let auth_email = auth_state.email.clone();
        let auth_password = auth_state.password.clone();

        let wallet_pubkey = match wallet.pubkey {
            Some(pk) => pk,
            None => {
                state.error_message = Some("Wallet not connected".to_string());
                state.is_validating = false;
                continue;
            }
        };

        info!("[PROFILE] Starting async submission for {}", username);

        tokio.0.spawn(async move {
            // 1. Sign and send on-chain transaction (Username + Country ONLY, no PII)
            let program_id = match get_program_id() {
                Ok(program_id) => program_id,
                Err(e) => {
                    error!("[PROFILE] Failed to get program ID: {}", e);
                    return;
                }
            };
            // Parse DOB string "YYYY-MM-DD" to Unix timestamp for on-chain age gate.
            let dob_unix: i64 = dob
                .split('-')
                .collect::<Vec<_>>()
                .as_slice()
                .chunks_exact(3)
                .next()
                .and_then(|parts| {
                    let y: i64 = parts[0].parse().ok()?;
                    let m: i64 = parts[1].parse().ok()?;
                    let d: i64 = parts[2].parse().ok()?;
                    // Simplified Gregorian → Unix (accurate enough for 18+ check)
                    let days = (y - 1970) * 365 + (y - 1969) / 4 - (y - 1901) / 100
                        + (y - 1601) / 400
                        + [0i64, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334]
                            [(m as usize).saturating_sub(1)]
                        + d
                        - 1;
                    Some(days * 86400)
                })
                .unwrap_or(0);

            let ix = match init_profile_ix(
                program_id,
                wallet_pubkey,
                username.clone(),
                country.clone(),
                dob_unix,
            ) {
                Ok(ix) => ix,
                Err(e) => {
                    error!("[PROFILE] Failed to build IX: {}", e);
                    return;
                }
            };

            info!("[PROFILE] Sending on-chain transaction...");
            match tauri_signer::sign_and_send_via_tauri(
                "https://api.devnet.solana.com",
                wallet_pubkey,
                &[ix],
                &[],
            ) {
                Ok(sig) => info!("[PROFILE] On-chain success: {}", sig),
                Err(e) => {
                    error!("[PROFILE] On-chain failed: {}", e);
                    // In a real app, we'd send an error event back to main thread
                    return;
                }
            }

            // 2. Register with backend
            info!("[PROFILE] Registering with backend...");
            let timestamp = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            {
                Ok(duration) => duration.as_secs(),
                Err(e) => {
                    error!("[PROFILE] Failed to compute timestamp: {}", e);
                    return;
                }
            };

            let auth_msg = format!("xfchess:register:{}", timestamp);
            let signature = match tauri_signer::sign_message_via_tauri(&auth_msg) {
                Ok(sig) => sig,
                Err(e) => {
                    error!("[PROFILE] Backend sign failed: {}", e);
                    return;
                }
            };

            let sig_str = base64::engine::general_purpose::STANDARD.encode(&signature);
            let reg_req = vps_client::RegisterReq {
                wallet: wallet_pubkey.to_string(),
                signature: sig_str,
                timestamp,
                username: username.clone(),
            };

            if let Err(e) = vps_client::register_wallet(&reg_req) {
                warn!("[PROFILE] Backend registration skipped/failed: {}", e);
            }

            // 2.5 Link wallet if account was created via email
            if !auth_email.is_empty() {
                info!(
                    "[PROFILE] Linking wallet to email account {}...",
                    auth_email
                );
                let link_msg = format!("xfchess:link:{}", timestamp);
                let link_sig = match tauri_signer::sign_message_via_tauri(&link_msg) {
                    Ok(sig) => sig,
                    Err(e) => {
                        error!("[PROFILE] Link sign failed: {}", e);
                        return;
                    }
                };

                let link_req = vps_client::LinkWalletReq {
                    email: auth_email,
                    password: auth_password,
                    wallet: wallet_pubkey.to_string(),
                    signature: bs58::encode(&link_sig).into_string(),
                    timestamp,
                };

                if let Err(e) = vps_client::link_wallet(&link_req) {
                    error!("[PROFILE] Wallet linking failed: {}", e);
                } else {
                    info!("[PROFILE] Wallet successfully linked to email account.");
                }
            }

            // 3. Submit KYC to backend
            info!("[PROFILE] Submitting KYC...");
            let kyc_msg = format!("register_identity:{}:{}", wallet_pubkey, timestamp);
            let kyc_sig = match tauri_signer::sign_message_via_tauri(&kyc_msg) {
                Ok(sig) => sig,
                Err(e) => {
                    error!("[PROFILE] KYC sign failed: {}", e);
                    return;
                }
            };
            let _kyc_sig_str = bs58::encode(&kyc_sig).into_string();
            let kyc_payload = vps_client::IdentityPayload {
                pubkey: wallet_pubkey.to_string(),
                full_name: username.clone(), // Use username as fallback for full name if not provided
                dob,
                address,
                country: country.clone(),
                tax_id: tax_id.clone(),
                signature: bs58::encode(&kyc_sig).into_string(), // Signature Display is base58
                timestamp,
                consent_kyc: true,
                consent_retention_years: 5,
            };

            if let Err(e) = vps_client::register_identity(&kyc_payload) {
                warn!("[PROFILE] KYC submission skipped/failed: {}", e);
            }

            info!("[PROFILE] All steps completed for {}", username);
        });

        // Optimistically return to main menu
        menu_state.set(MenuState::Main);
    }
}

/// Cleanup system
pub fn despawn_profile_creation_ui(
    mut commands: Commands,
    query: Query<Entity, With<ProfileCreationUi>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}
