//! First-run local profile onboarding.
//!
//! Shown once, only when no local profile name has ever been saved
//! (`Documents/xfchess/guest_username` doesn't exist yet, per
//! `load_local_profile_on_startup` in `states/main_menu.rs`) and no wallet
//! is connected. Confirming saves the name locally via
//! `multiplayer::network::identity::save_guest_username` and populates
//! `PlayerIdentity` immediately — every later launch reads it back silently,
//! with no prompt.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::states::main_menu::PlayerIdentity;
use crate::ui::styles::StyledPanel;

const MAX_NAME_LEN: usize = 20;

#[derive(Resource, Default)]
pub struct ProfileOnboardingState {
    draft: String,
    error: Option<String>,
}

pub struct ProfileOnboardingPlugin;

impl Plugin for ProfileOnboardingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ProfileOnboardingState>()
            .add_systems(bevy_egui::EguiPrimaryContextPass, draw_profile_onboarding);
    }
}

fn draw_profile_onboarding(
    mut contexts: EguiContexts,
    mut player_identity: ResMut<PlayerIdentity>,
    mut state: ResMut<ProfileOnboardingState>,
    #[cfg(feature = "solana")] solana_state: Option<
        Res<crate::multiplayer::solana::integration::state::SolanaIntegrationState>,
    >,
) {
    // A name already exists — persisted local profile, or a wallet is
    // already connected and will populate one shortly — nothing to prompt.
    if player_identity.username.is_some() {
        return;
    }
    #[cfg(feature = "solana")]
    if solana_state.as_ref().and_then(|s| s.wallet_pubkey).is_some() {
        return;
    }

    let Ok(ctx) = contexts.ctx_mut() else { return };

    let mut confirmed = false;

    egui::Window::new("profile_onboarding")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size([340.0, 220.0])
        .frame(StyledPanel::popup())
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("Welcome to XFChess")
                        .size(20.0)
                        .family(egui::FontFamily::Name("CinzelBold".into())),
                );
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(
                        "Choose a name, saved locally on this device. Used for Computer \
                         games and local online games — connecting a Solana wallet later \
                         uses your on-chain profile name instead.",
                    )
                    .size(12.0)
                    .color(egui::Color32::from_rgb(160, 160, 175)),
                );
                ui.add_space(16.0);

                let resp = ui.add(
                    egui::TextEdit::singleline(&mut state.draft)
                        .hint_text("Your name")
                        .char_limit(MAX_NAME_LEN)
                        .desired_width(240.0),
                );
                let enter_pressed =
                    resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                if let Some(err) = state.error.clone() {
                    ui.add_space(6.0);
                    ui.colored_label(egui::Color32::from_rgb(255, 100, 100), err);
                }

                ui.add_space(16.0);
                if ui
                    .add_sized(
                        [160.0, 38.0],
                        egui::Button::new(
                            egui::RichText::new("Continue")
                                .size(14.0)
                                .strong()
                                .color(egui::Color32::from_rgb(20, 18, 10)),
                        )
                        .fill(egui::Color32::from_rgb(244, 187, 68)),
                    )
                    .clicked()
                    || enter_pressed
                {
                    confirmed = true;
                }
            });
        });

    if confirmed {
        let trimmed = state.draft.trim();
        if trimmed.is_empty() {
            state.error = Some("Enter a name to continue.".to_string());
        } else {
            crate::multiplayer::network::identity::save_guest_username(trimmed);
            player_identity.username = Some(trimmed.to_string());
            player_identity.is_guest = true;
            state.error = None;
        }
    }
}
