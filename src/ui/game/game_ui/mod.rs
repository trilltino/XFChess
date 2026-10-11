//! In-game HUD panels: player cards and avatars ([`players`]), the move list
//! ([`moves`]), clocks ([`clock`]), banners and offers ([`banners`]), and the
//! tournament sidebar (Solana builds).

use crate::core::GameMode;
use crate::game::components::GamePhase;
use crate::game::resources::system_params::GameStateParams;
use crate::game::view_mode::ViewMode;
use crate::rendering::pieces::PieceColor;
use crate::ui::styles::*;
use crate::ui::system_params::GameUIParams;
use bevy::prelude::*;
use bevy_egui::egui;
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::collections::{HashMap, VecDeque};

mod banners;
mod clock;
mod moves;
mod players;
#[cfg(feature = "solana")]
mod tournament_sidebar;
pub use banners::*;
pub use clock::*;
pub(crate) use moves::*;
pub use players::*;
#[cfg(feature = "solana")]
pub use tournament_sidebar::*;

#[derive(Resource, Default)]
pub struct InGameHudVisibility {
    pub visible: bool,
}

pub fn reset_in_game_hud_visibility(mut hud_visibility: ResMut<InGameHudVisibility>) {
    hud_visibility.visible = true;
}

pub fn toggle_in_game_hud(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut hud_visibility: ResMut<InGameHudVisibility>,
) {
    if keyboard.just_pressed(KeyCode::KeyB) {
        hud_visibility.visible = !hud_visibility.visible;
    }
}

pub fn in_game_hud_visible(hud_visibility: Res<InGameHudVisibility>) -> bool {
    hud_visibility.visible
}

pub fn game_status_ui(mut params: GameUIParams) {
    if !params.hud_visibility.visible {
        return;
    }

    // PGN replay has its own right panel in replay.rs — skip here.
    if *params.game_mode == GameMode::PgnReplay {
        return;
    }

    // Clone the Arc-backed egui context to release the params borrow for panel closures.
    let ctx = {
        let Ok(ctx_ref) = params.contexts.ctx_mut() else {
            return;
        };
        ctx_ref.clone()
    };

    // Decode any pending avatar bytes into egui textures.
    params.avatar_cache.flush_pending(&ctx);

    // Declare the full-width top bar before side panels consume the remaining egui rect.
    crate::ui::game::top_bar::render_game_top_bar(&ctx, &mut params);

    // Resolve player bars once into a cache shared by 3D overlays and the 2D board.
    let (top_bar_data, bottom_bar_data) =
        crate::ui::game::player_bar::build_player_bar_data(&params);
    params.player_bars_cache.top = top_bar_data.clone();
    params.player_bars_cache.bottom = bottom_bar_data.clone();
    if *params.view_mode != crate::game::view_mode::ViewMode::Standard2D {
        crate::ui::game::player_bar::render_center_player_bars_3d(
            &ctx,
            &top_bar_data,
            &bottom_bar_data,
            crate::ui::game::top_bar::TOP_BAR_HEIGHT,
        );
    }

    match params.game_state.game_phase.0 {
        GamePhase::Checkmate => render_checkmate_banner(
            &ctx,
            &params.game_state,
            params.view_mode.as_ref(),
            params.board_2d_layout.rect,
        ),
        GamePhase::Check => render_check_banner(&ctx),
        _ => {}
    }

    if params.exit_confirmation.visible && !params.game_state.game_over.is_game_over() {
        let is_online = matches!(
            *params.game_mode,
            GameMode::OnlineMultiplayer | GameMode::MultiplayerCompetitive
        );
        let confirmation_text = if is_online {
            "Are you sure you want to exit? If you leave an online game, it will be forfeited."
        } else {
            "Are you sure you want to exit this game?"
        };

        egui::Window::new("exit_game_confirmation")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([420.0, 200.0])
            .frame(StyledPanel::popup())
            .show(&ctx, |ui| {
                ui.set_width(368.0);
                ui.vertical_centered(|ui| {
                    ui.label(TextStyle::popup_title("EXIT GAME"));
                    ui.add_space(10.0);
                    ui.label(TextStyle::popup_body(confirmation_text));
                    ui.add_space(18.0);
                    // horizontal_centered only centers vertically — pad manually
                    // so the button pair sits centered in the popup.
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 14.0;
                        let buttons_width = 120.0 * 2.0 + 14.0;
                        ui.add_space(((ui.available_width() - buttons_width) / 2.0).max(0.0));
                        if ui
                            .add_sized(
                                [120.0, 40.0],
                                egui::Button::new(
                                    egui::RichText::new("No")
                                        .size(13.0)
                                        .color(egui::Color32::WHITE),
                                )
                                .fill(UiColors::BTN_POPUP_DARK)
                                .stroke(egui::Stroke::NONE)
                                .corner_radius(8.0),
                            )
                            .clicked()
                        {
                            params.exit_confirmation.visible = false;
                            params.exit_confirmation.pending_exit = false;
                        }

                        if ui
                            .add_sized(
                                [120.0, 40.0],
                                egui::Button::new(
                                    egui::RichText::new("Yes")
                                        .size(13.0)
                                        .color(egui::Color32::WHITE)
                                        .strong(),
                                )
                                .fill(UiColors::DANGER)
                                .stroke(egui::Stroke::NONE)
                                .corner_radius(8.0),
                            )
                            .clicked()
                        {
                            params.exit_confirmation.pending_exit = true;
                        }
                    });
                });
            });
    }

    // Declare the left panel before the board so the central panel reserves its space.
    egui::Panel::left("game_info_panel")
        .resizable(false)
        .show_separator_line(false)
        .exact_size(Layout::SIDE_PANEL_WIDTH)
        .frame(
            egui::Frame::default()
                .fill(UiColors::BG_OVERLAY)
                .inner_margin(egui::Margin::symmetric(8, 10))
                .stroke(egui::Stroke::NONE),
        )
        .show(
            &mut crate::ui::egui_ext::viewport_ui(&ctx, "game_info_panel"),
            |ui| {
                crate::ui::game::left_panel::render_game_left_panel(ui, &mut params);
            },
        );

    // Declare the Solana sidebar first to place it rightmost; game_panel stays beside the board.

    #[cfg(feature = "solana")]
    if *params.game_mode == GameMode::MultiplayerCompetitive {
        if let (Some(ref mut wallet), Some(ref mut sync), Some(ref mut comp), Some(ref profile)) = (
            params.solana_wallet.as_mut(),
            params.solana_sync.as_mut(),
            params.competitive_match.as_mut(),
            params.solana_profile.as_ref(),
        ) {
            let solana_integration = params.solana_integration.as_ref();
            let profile_view = params.profile_view.as_mut();
            egui::Panel::right("solana_sidebar")
                .resizable(true)
                .default_size(250.0)
                .show_separator_line(false)
                .show(
                    &mut crate::ui::egui_ext::viewport_ui(&ctx, "solana_sidebar"),
                    |ui| {
                        let sol_usd_rate = params
                            .sol_usd_rate
                            .as_ref()
                            .and_then(|r| r.snapshot())
                            .map(|s| s.usd_per_sol);
                        egui::ScrollArea::vertical()
                            .max_height(f32::INFINITY)
                            .show(ui, |ui| {
                                if let Some(si) = solana_integration {
                                    let mut pv_open =
                                        profile_view.as_ref().map(|pv| pv.open).unwrap_or(false);
                                    #[cfg(feature = "solana")]
                                    let gs = params.global_session_active.as_deref();
                                    #[cfg(not(feature = "solana"))]
                                    let gs = None;
                                    #[cfg(feature = "solana")]
                                    let gs_pending = params.global_session_pending.is_some();
                                    #[cfg(not(feature = "solana"))]
                                    let gs_pending = false;
                                    crate::ui::solana_panel::render_solana_panel(
                                        ui,
                                        wallet,
                                        sync,
                                        comp,
                                        profile,
                                        si,
                                        &mut pv_open,
                                        gs,
                                        gs_pending,
                                        sol_usd_rate,
                                        params.solana_session.as_deref(),
                                    );
                                    if let Some(pv) = profile_view {
                                        pv.open = pv_open;
                                    }
                                } else {
                                    crate::ui::solana_panel::render_solana_panel(
                                        ui,
                                        wallet,
                                        sync,
                                        comp,
                                        profile,
                                        &Default::default(),
                                        &mut false,
                                        None,
                                        false,
                                        sol_usd_rate,
                                        params.solana_session.as_deref(),
                                    );
                                }
                            });
                    },
                );
        }
    }
}

pub fn play_check_sound_system(
    mut commands: Commands,
    game_phase: Res<crate::game::resources::CurrentGamePhase>,
    sounds: Option<Res<crate::game::resources::GameSounds>>,
    settings: Res<crate::core::GameSettings>,
) {
    use crate::game::components::GamePhase;
    if !game_phase.is_changed() {
        return;
    }
    if settings.muted {
        return;
    }
    let Some(s) = sounds else { return };
    if game_phase.0 == GamePhase::Check {
        crate::game::resources::sounds::play_sfx(&mut commands, s.check.clone());
    }
}

pub fn toggle_blindfold_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut settings: ResMut<crate::core::GameSettings>,
) {
    if keyboard.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight])
        && keyboard.just_pressed(KeyCode::KeyB)
    {
        settings.blindfold = !settings.blindfold;
    }
}
