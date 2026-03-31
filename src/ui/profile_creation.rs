//! Profile Creation UI - Username selection screen
//!
//! Implements the "Option 2: Manual Username Selection" flow from
//! the design document at logs/profile-creation-option2-090cd2.md

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::core::states::{DespawnOnExit, MenuState};
use crate::multiplayer::solana::addon::SolanaWallet;

/// Resource tracking profile creation state
#[derive(Resource, Default)]
pub struct ProfileCreationState {
    pub username_input: String,
    pub availability_status: UsernameAvailability,
    pub is_validating: bool,
    pub error_message: Option<String>,
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

/// System that renders the profile creation UI
pub fn profile_creation_ui_system(
    mut contexts: EguiContexts,
    mut state: ResMut<ProfileCreationState>,
    mut menu_state: ResMut<NextState<MenuState>>,
    wallet: Res<SolanaWallet>,
) {
    let ctx = contexts.ctx_mut().expect("Failed to get Egui context");
    let screen_size = ctx.screen_rect().size();
    
    // Centered panel
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            let center_x = screen_size.x / 2.0;
            let center_y = screen_size.y / 2.0;
            
            // Calculate panel position (centered, 400x400)
            let panel_width = 400.0;
            let panel_rect = egui::Rect::from_center_size(
                egui::pos2(center_x, center_y),
                egui::vec2(panel_width, 400.0),
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
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(panel_rect.shrink(32.0)), |ui| {
                ui.vertical_centered(|ui| {
                    // Header
                    ui.add_space(20.0);
                    ui.heading(egui::RichText::new("⚡ Create Your Profile")
                        .color(egui::Color32::from_rgb(230, 57, 70))
                        .size(28.0));
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Choose a unique username to get started")
                        .color(egui::Color32::GRAY)
                        .size(14.0));
                    ui.add_space(30.0);
                    
                    // Wallet info display
                    if let Some(ref pubkey) = wallet.pubkey {
                        let pk_str = pubkey.to_string();
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
                    let input_response = ui.add(
                        egui::TextEdit::singleline(&mut state.username_input)
                            .hint_text("Enter username...")
                            .char_limit(20)
                            .min_size(egui::vec2(280.0, 48.0))
                            .font(egui::FontId::proportional(18.0))
                            .margin(egui::Margin::symmetric(16, 12))
                    );
                    
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
                    let can_submit = !state.username_input.is_empty() 
                        && state.availability_status == UsernameAvailability::Available
                        && !state.is_validating;
                    
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
                        // TODO: Submit transaction to set username
                        info!("[PROFILE] Creating profile with username: {}", state.username_input);
                        state.is_validating = true;
                        // Transition back to main menu after success
                        menu_state.set(MenuState::Main);
                    }
                    
                    ui.add_space(16.0);
                    
                    // Skip button
                    let skip_response = ui.add(
                        egui::Button::new(
                            egui::RichText::new("Skip for now")
                                .color(egui::Color32::GRAY)
                                .size(13.0)
                        )
                        .frame(false)
                    );
                    
                    if skip_response.clicked() {
                        menu_state.set(MenuState::Main);
                    }
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

/// Spawn profile creation UI
pub fn spawn_profile_creation_ui(mut commands: Commands) {
    commands.spawn((
        ProfileCreationUi,
        DespawnOnExit(MenuState::ProfileCreation),
    ));
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
