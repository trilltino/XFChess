#![allow(dead_code)]
//! Profile Creation UI - Username selection screen
//!
//! the design document at logs/profile-creation-option2-090cd2.md

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::core::states::{DespawnOnExit, MenuState};
use crate::multiplayer::solana::addon::SolanaWallet;
use crate::multiplayer::solana::tauri_signer;
use crate::multiplayer::TokioRuntime;
use crate::solana::instructions::{init_profile_ix, get_program_id};
use crate::multiplayer::vps_client;
use base64::Engine;

/// Resource tracking profile creation state
#[derive(Resource, Default)]
pub struct ProfileCreationState {
    pub username_input: String,
    pub availability_status: UsernameAvailability,
    pub is_validating: bool,
    pub error_message: Option<String>,
    pub country_code: String,           // ISO 3166-1 alpha-2 (e.g., "GB", "BR", "CA", "DE")
    pub tax_id: String,                 // Country-specific tax ID
    pub tax_id_valid: bool,            // Tax ID format validation status
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

/// System that renders the profile creation UI
pub fn profile_creation_ui_system(
    mut contexts: EguiContexts,
    mut state: ResMut<ProfileCreationState>,
    _menu_state: ResMut<NextState<MenuState>>,
    wallet: Res<SolanaWallet>,
    mut submission_events: MessageWriter<ProfileSubmissionEvent>,
) {
    let ctx = contexts.ctx_mut().expect("Failed to get Egui context");
    let screen_size = ctx.content_rect().size();
    
    // Centered panel
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            let center_x = screen_size.x / 2.0;
            let center_y = screen_size.y / 2.0;
            
            // Calculate panel position (centered, 400x600)
            let panel_width = 400.0;
            let panel_rect = egui::Rect::from_center_size(
                egui::pos2(center_x, center_y),
                egui::vec2(panel_width, 600.0),
            );
            
            // Semi-transparent background
            ui.painter().rect_filled(
                panel_rect,
                16.0,
                egui::Color32::from_rgba_premultiplied(20, 20, 25, 245),
            );
            
            // Border
            ui.painter().rect_stroke(
                panel_rect,
                16.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(230, 57, 70)),
                egui::StrokeKind::Outside,
            );
            
            // Content area
            ui.scope_builder(egui::UiBuilder::new().max_rect(panel_rect.shrink(32.0)), |ui| {
                ui.vertical_centered(|ui| {
                    // Header
                    ui.add_space(20.0);
                    ui.heading(egui::RichText::new("⚡ Create Username on SOL")
                        .color(egui::Color32::from_rgb(230, 57, 70))
                        .size(28.0));
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Choose a unique username to get started")
                        .color(egui::Color32::GRAY)
                        .size(14.0));
                    ui.add_space(30.0);
                    
                    // Wallet info display
                    if let Some(ref pubkey) = wallet.pubkey {
                        let pk_str = format!("{}", pubkey);
                        ui.label(egui::RichText::new(format!(
                            "Wallet: {}...{}",
                            &pk_str[..6],
                            &pk_str[pk_str.len()-4..]
                        ))
                        .color(egui::Color32::from_rgb(100, 100, 110))
                        .monospace()
                        .size(12.0));
                        ui.add_space(20.0);
                    }
                    
                    // Username input field
                    ui.add(
                        egui::TextEdit::singleline(&mut state.username_input)
                            .hint_text("Enter username...")
                            .char_limit(20)
                            .min_size(egui::vec2(280.0, 48.0))
                            .font(egui::FontId::proportional(18.0))
                            .margin(egui::Margin::symmetric(16, 12))
                    );
                    
                    ui.add_space(24.0);
                    
                    // Country selection
                    ui.label(egui::RichText::new("Country (for fee calculation)")
                        .color(egui::Color32::from_rgb(150, 150, 160))
                        .size(13.0));
                    ui.add_space(8.0);
                    
                    let countries = [
                        ("GB", "United Kingdom"),
                        ("BR", "Brazil"),
                        ("CA", "Canada"),
                        ("DE", "Germany"),
                        ("US", "United States"),
                        ("SG", "Singapore"),
                        ("CH", "Switzerland"),
                        ("AE", "United Arab Emirates"),
                        ("HK", "Hong Kong"),
                        ("JP", "Japan"),
                        ("MT", "Malta"),
                        ("GI", "Gibraltar"),
                        ("IS", "Iceland"),
                        ("LI", "Liechtenstein"),
                        ("MC", "Monaco"),
                        ("Other", "Other"),
                    ];
                    
                    egui::ComboBox::from_id_salt("country_selector")
                        .selected_text(if state.country_code.is_empty() {
                            "Select country..."
                        } else {
                            countries.iter()
                                .find(|(code, _)| *code == state.country_code)
                                .map(|(_, name)| *name)
                                .unwrap_or("Other")
                        })
                        .width(280.0)
                        .show_ui(ui, |ui| {
                            for (code, name) in &countries {
                                ui.selectable_value(&mut state.country_code, code.to_string(), *name);
                            }
                        });
                    
                    ui.add_space(16.0);
                    
                    // Tax ID input (conditional on country)
                    if !state.country_code.is_empty() && state.country_code != "Other" {
                        ui.label(egui::RichText::new("Tax ID (required for compliance)")
                            .color(egui::Color32::from_rgb(150, 150, 160))
                            .size(13.0));
                        ui.add_space(8.0);
                        
                        let tax_id_hint = match state.country_code.as_str() {
                            "GB" => "National Insurance Number (NI)",
                            "BR" => "CPF (Cadastro de Pessoas Físicas)",
                            "CA" => "Social Insurance Number (SIN)",
                            "DE" => "Tax ID (Steueridentifikationsnummer)",
                            "US" => "Social Security Number (SSN) - Optional",
                            "SG" => "NRIC/FIN - Optional",
                            "CH" => "Tax ID (AHV) - Optional",
                            "AE" => "Emirates ID - Optional",
                            "HK" => "Hong Kong ID - Optional",
                            "JP" => "My Number - Optional",
                            "MT" => "Tax ID - Optional",
                            "GI" => "Tax ID - Optional",
                            "IS" => "Tax ID (kennitala) - Optional",
                            "LI" => "Tax ID - Optional",
                            "MC" => "Tax ID - Optional",
                            _ => "Tax ID (Optional)",
                        };
                        
                        ui.add(
                            egui::TextEdit::singleline(&mut state.tax_id)
                                .hint_text(tax_id_hint)
                                .char_limit(50)
                                .min_size(egui::vec2(280.0, 48.0))
                                .font(egui::FontId::proportional(18.0))
                                .margin(egui::Margin::symmetric(16, 12))
                        );
                        
                        ui.add_space(8.0);
                        
                        // Tax ID validation feedback
                        let tax_id_valid = validate_tax_id_format(&state.country_code, &state.tax_id);
                        if !state.tax_id.is_empty() {
                            let (text, color) = if tax_id_valid {
                                ("✓ Valid format", egui::Color32::from_rgb(34, 197, 94))
                            } else {
                                ("✗ Invalid format", egui::Color32::from_rgb(239, 68, 68))
                            };
                            ui.label(egui::RichText::new(text).color(color).size(12.0));
                            state.tax_id_valid = tax_id_valid;
                        }
                        
                        ui.add_space(16.0);
                    }

                    // Email, DOB, Address inputs
                    ui.label(egui::RichText::new("Identity Details (Backend Only)")
                        .color(egui::Color32::from_rgb(150, 150, 160))
                        .size(13.0));
                    ui.add_space(8.0);

                    ui.add(egui::TextEdit::singleline(&mut state.email)
                        .hint_text("Email Address")
                        .min_size(egui::vec2(280.0, 32.0)));
                    ui.add_space(8.0);

                    ui.add(egui::TextEdit::singleline(&mut state.dob)
                        .hint_text("Date of Birth (YYYY-MM-DD)")
                        .min_size(egui::vec2(280.0, 32.0)));
                    ui.add_space(8.0);

                    ui.add(egui::TextEdit::singleline(&mut state.address)
                        .hint_text("Residential Address")
                        .min_size(egui::vec2(280.0, 32.0)));
                    ui.add_space(20.0);

                    // Wallet Connection Section
                    ui.label(egui::RichText::new("Solana Wallet (Required for Wagers)")
                        .color(egui::Color32::from_rgb(150, 150, 160))
                        .size(13.0));
                    ui.add_space(8.0);

                    if let Some(pubkey) = wallet.pubkey {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("✓ Connected:").color(egui::Color32::from_rgb(34, 197, 94)));
                            let pk_str = pubkey.to_string();
                            ui.label(egui::RichText::new(format!("{}...{}", &pk_str[..6], &pk_str[pk_str.len()-4..]))
                                .monospace()
                                .color(egui::Color32::WHITE));
                        });
                    } else {
                        ui.horizontal(|ui| {
                            if ui.button(egui::RichText::new("👻 Phantom").strong()).clicked() {
                                crate::multiplayer::solana::tauri_signer::open_wallet_browser();
                            }
                            ui.add_space(8.0);
                            if ui.button(egui::RichText::new("☀️ Solflare").strong()).clicked() {
                                crate::multiplayer::solana::tauri_signer::open_wallet_browser();
                            }
                        });
                        ui.label(egui::RichText::new("Please connect your wallet to proceed.").color(egui::Color32::from_rgb(200, 100, 100)).size(11.0));
                    }

                    ui.add_space(24.0);
                    
                    // Validation feedback
                    ui.add_space(12.0);
                    let status_text = match state.availability_status {
                        UsernameAvailability::Unknown => ("", egui::Color32::GRAY),
                        UsernameAvailability::Checking => ("⏳ Checking availability...", egui::Color32::YELLOW),
                        UsernameAvailability::Available => ("✓ Username available!", egui::Color32::from_rgb(34, 197, 94)),
                        UsernameAvailability::Taken => ("✗ Username already taken", egui::Color32::from_rgb(239, 68, 68)),
                        UsernameAvailability::Invalid => ("✗ Invalid username format", egui::Color32::from_rgb(239, 68, 68)),
                    };
                    ui.label(egui::RichText::new(status_text.0).color(status_text.1).size(13.0));
                    
                    // Error message
                    if let Some(ref error) = state.error_message {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(error)
                            .color(egui::Color32::from_rgb(239, 68, 68))
                            .size(12.0));
                    }
                    
                    // Rules reminder
                    ui.add_space(16.0);
                    ui.label(egui::RichText::new("3-20 chars • A-Z, a-z, 0-9, _, -")
                        .color(egui::Color32::from_rgb(80, 80, 90))
                        .size(11.0));
                    
                    ui.add_space(30.0);
                    
                    // Create Profile button
                    let wallet_connected = wallet.is_connected();
                    let compliance_countries = ["GB", "BR", "CA", "DE"]; // Countries requiring tax ID
                    let requires_tax_id = compliance_countries.contains(&state.country_code.as_str());
                    let tax_id_valid = !requires_tax_id || state.tax_id_valid;
                    
                    let can_submit = wallet_connected
                        && !state.username_input.is_empty()
                        && !state.country_code.is_empty()
                        && state.availability_status == UsernameAvailability::Available
                        && tax_id_valid
                        && !state.is_validating;
                    
                    if !wallet_connected {
                        ui.label(egui::RichText::new("Please connect your wallet first")
                            .color(egui::Color32::from_rgb(239, 68, 68))
                            .size(12.0));
                        ui.add_space(8.0);
                    }
                    
                    let button_color = if can_submit {
                        egui::Color32::from_rgb(230, 57, 70)
                    } else {
                        egui::Color32::from_rgb(80, 80, 90)
                    };
                    
                    let button_response = ui.add_sized(
                        egui::vec2(280.0, 48.0),
                        egui::Button::new(
                            egui::RichText::new("Create Profile")
                                .color(egui::Color32::WHITE)
                                .size(16.0)
                                .strong()
                        )
                        .fill(button_color)
                        .corner_radius(8.0)
                    );
                    
                    if button_response.clicked() && can_submit {
                        info!("[PROFILE] Triggering profile submission for: {}", state.username_input);
                        state.is_validating = true;
                        submission_events.write(ProfileSubmissionEvent {
                            username: state.username_input.clone(),
                            country: state.country_code.clone(),
                            tax_id: state.tax_id.clone(),
                            email: state.email.clone(),
                            dob: state.dob.clone(),
                            address: state.address.clone(),
                        });
                    }
                    
                    ui.add_space(16.0);
                });
            });
        });
}

/// System to validate username input
pub fn validate_username_system(
    mut state: ResMut<ProfileCreationState>,
    mut last_checked: Local<String>,
) {
    let username = state.username_input.clone();
    
    // Skip if unchanged or empty
    if username.is_empty() || username == *last_checked {
        if username.is_empty() {
            state.availability_status = UsernameAvailability::Unknown;
        }
        return;
    }
    
    // Validate format
    if !is_valid_username_format(&username) {
        state.availability_status = UsernameAvailability::Invalid;
        *last_checked = username;
        return;
    }
    
    // Mark as checking (in real implementation, this would query on-chain)
    state.availability_status = UsernameAvailability::Checking;
    *last_checked = username;
    
    // TODO: Query on-chain to check if username is taken
    // For now, simulate availability check
    // In production, this should query the UsernameRecord PDA
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
    let reserved = ["admin", "system", "support", "official", "moderator",
                    "xf", "xfchess", "chess", "test", "dev", "null"];
    for r in reserved {
        if lower == r || lower.starts_with(r) {
            return false;
        }
    }
    
    true
}

/// Validate tax ID format based on country code
fn validate_tax_id_format(country: &str, tax_id: &str) -> bool {
    // For optional countries, empty tax_id is valid
    let optional_countries = ["US", "SG", "CH", "AE", "HK", "JP", "MT", "GI", "IS", "LI", "MC", "Other"];
    if optional_countries.contains(&country) {
        return true; // Optional for friendly territories
    }
    
    // For required compliance countries, tax_id must not be empty
    if tax_id.is_empty() {
        return false;
    }
    
    match country {
        "GB" => {
            // UK National Insurance Number: 2 letters + 6 digits + 1 letter (e.g., AB123456C)
            let pattern = regex::Regex::new(r"^[A-Z]{2}\d{6}[A-Z]$").unwrap();
            pattern.is_match(tax_id)
        }
        "BR" => {
            // Brazil CPF: 11 digits with optional formatting (e.g., 123.456.789-00)
            let digits: String = tax_id.chars().filter(|c| c.is_ascii_digit()).collect();
            digits.len() == 11
        }
        "CA" => {
            // Canada SIN: 9 digits (e.g., 123456789)
            let pattern = regex::Regex::new(r"^\d{9}$").unwrap();
            pattern.is_match(tax_id)
        }
        "DE" => {
            // Germany Tax ID: 11 digits (e.g., 12345678901)
            let pattern = regex::Regex::new(r"^\d{11}$").unwrap();
            pattern.is_match(tax_id)
        }
        _ => true, // No strict validation for other countries
    }
}

/// Spawn profile creation UI
pub fn spawn_profile_creation_ui(mut commands: Commands) {
    commands.spawn((
        ProfileCreationUi,
        DespawnOnExit(MenuState::ProfileCreation),
    ));
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
) {
    for event in events.read() {
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
            let program_id = get_program_id().unwrap();
            let ix = match init_profile_ix(program_id, wallet_pubkey, username.clone(), country.clone()) {
                Ok(ix) => ix,
                Err(e) => {
                    error!("[PROFILE] Failed to build IX: {}", e);
                    return;
                }
            };

            info!("[PROFILE] Sending on-chain transaction...");
            match tauri_signer::sign_and_send_via_tauri("https://api.devnet.solana.com", wallet_pubkey, &[ix], &[]) {
                Ok(sig) => info!("[PROFILE] On-chain success: {}", sig),
                Err(e) => {
                    error!("[PROFILE] On-chain failed: {}", e);
                    // In a real app, we'd send an error event back to main thread
                    return;
                }
            }

            // 2. Register with backend
            info!("[PROFILE] Registering with backend...");
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            
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
                info!("[PROFILE] Linking wallet to email account {}...", auth_email);
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
