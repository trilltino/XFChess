//! Local profile picker, shown on launch until a profile is active.
//!
//! Several people can share one machine. Every profile ever created on this
//! device (`identity::list_profiles`) is offered as a clickable entry;
//! picking one activates it immediately via `identity::set_active_profile`.
//! A "+" entry reveals a name field to create a new profile
//! (`identity::create_profile`), which also gets its own PGN save subfolder
//! (`identity::profile_pgn_dir`). Confirming either path populates
//! `PlayerIdentity` immediately — every later launch loads the last-active
//! profile silently, with no prompt, unless a wallet connects instead.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::multiplayer::network::identity;
use crate::states::main_menu::PlayerIdentity;
use crate::ui::styles::StyledPanel;

const MAX_NAME_LEN: usize = 20;

#[derive(Resource, Default)]
pub struct ProfileOnboardingState {
    draft: String,
    error: Option<String>,
    /// Show the "new profile" name field instead of the profile list. Forced
    /// true automatically when no profiles exist yet.
    creating: bool,
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
    // A profile is already active, or a wallet is already connected and will
    // populate one shortly — nothing to prompt.
    if player_identity.username.is_some() {
        return;
    }
    #[cfg(feature = "solana")]
    if solana_state
        .as_ref()
        .and_then(|s| s.wallet_pubkey)
        .is_some()
    {
        return;
    }

    let Ok(ctx) = contexts.ctx_mut() else { return };

    let existing = identity::list_profiles();
    // No profiles yet on this device — skip straight to the name field.
    if existing.is_empty() {
        state.creating = true;
    }

    let mut activate: Option<String> = None;
    let mut delete_target: Option<String> = None;
    let mut confirmed_new = false;
    let mut back_to_list = false;

    egui::Window::new("profile_onboarding")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size([360.0, if state.creating { 220.0 } else { 260.0 }])
        .frame(StyledPanel::popup())
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("Welcome to XFChess")
                        .size(20.0)
                        .family(egui::FontFamily::Name("CinzelBold".into())),
                );
                ui.add_space(6.0);

                if state.creating {
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
                        confirmed_new = true;
                    }

                    if !existing.is_empty() {
                        ui.add_space(8.0);
                        if ui.link("Back to profile list").clicked() {
                            back_to_list = true;
                        }
                    }
                } else {
                    ui.label(
                        egui::RichText::new("Pick a local profile, or create a new one.")
                            .size(12.0)
                            .color(egui::Color32::from_rgb(160, 160, 175)),
                    );
                    ui.add_space(12.0);

                    // `ui.horizontal` spans the full available width of its
                    // parent, so a two-button row inside `vertical_centered`
                    // still renders flush left — only a leaf widget's own
                    // rect gets centered. `allocate_ui` with the row's real
                    // content width turns the row into a leaf-sized block
                    // that centers correctly, matching the "+ New profile"
                    // button below it.
                    const ROW_WIDTH: f32 = 208.0 + 8.0 + 34.0;
                    for name in &existing {
                        ui.allocate_ui(egui::vec2(ROW_WIDTH, 34.0), |ui| {
                            ui.horizontal(|ui| {
                                if ui
                                    .add_sized(
                                        [208.0, 34.0],
                                        egui::Button::new(egui::RichText::new(name).size(14.0)),
                                    )
                                    .clicked()
                                {
                                    activate = Some(name.clone());
                                }
                                if ui
                                    .add_sized(
                                        [34.0, 34.0],
                                        egui::Button::new(
                                            egui::RichText::new("X")
                                                .color(egui::Color32::from_rgb(255, 60, 60))
                                                .strong()
                                                .size(14.0),
                                        ),
                                    )
                                    .clicked()
                                {
                                    delete_target = Some(name.clone());
                                }
                            });
                        });
                        ui.add_space(4.0);
                    }

                    ui.add_space(8.0);
                    if ui
                        .add_sized(
                            [ROW_WIDTH, 34.0],
                            egui::Button::new(
                                egui::RichText::new("+ New profile")
                                    .size(14.0)
                                    .strong()
                                    .color(egui::Color32::from_rgb(20, 18, 10)),
                            )
                            .fill(egui::Color32::from_rgb(244, 187, 68)),
                        )
                        .clicked()
                    {
                        state.creating = true;
                        state.draft.clear();
                        state.error = None;
                    }
                }
            });
        });

    if let Some(name) = activate {
        identity::set_active_profile(&name);
        player_identity.username = Some(name);
        player_identity.is_guest = true;
    }

    if back_to_list {
        state.creating = false;
        state.error = None;
    }

    if confirmed_new {
        let trimmed = state.draft.trim();
        if trimmed.is_empty() {
            state.error = Some("Enter a name to continue.".to_string());
        } else if existing.iter().any(|n| n == trimmed) {
            state.error = Some("That name is already taken on this device.".to_string());
        } else {
            let folder = rfd::FileDialog::new()
                .set_title("Choose Save Location")
                .pick_folder();
            identity::create_profile(trimmed, folder);
            player_identity.username = Some(trimmed.to_string());
            player_identity.is_guest = true;
            state.error = None;
        }
    }

    if let Some(name) = delete_target {
        identity::delete_profile(&name);
    }
}
