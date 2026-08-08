//! Top HUD bar for the in-game screen: 2D/3D view toggle on the right.
//! Declared first among the panels in `game_status_ui` so it reserves the
//! full-width strip before the left/right sidebars and the board area are
//! laid out beneath it.

use crate::game::view_mode::ViewMode;
use crate::ui::styles::*;
use bevy_egui::egui;

/// Fixed height of the top bar — also used by the 3D center player bars
/// (`player_bar::render_center_player_bars_3d`) to offset below it.
pub const TOP_BAR_HEIGHT: f32 = 44.0;

/// Renders the top bar and handles the 2D/3D toggle click. Called from
/// `game_status_ui` before any `SidePanel` so it spans the full window width.
pub fn render_game_top_bar(
    ctx: &egui::Context,
    params: &mut crate::ui::system_params::game_ui::GameUIParams,
) {
    egui::TopBottomPanel::top("game_top_bar")
        .exact_height(TOP_BAR_HEIGHT)
        .resizable(false)
        .show_separator_line(false)
        .frame(
            egui::Frame::default()
                .fill(UiColors::BG_OVERLAY)
                .inner_margin(egui::Margin::symmetric(16, 0)),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let is_3d = *params.view_mode != ViewMode::Standard2D;
                    if segmented_toggle(ui, "2D VIEW", "3D VIEW", is_3d).is_some() {
                        params.view_mode.toggle();
                    }

                    ui.add_space(10.0);

                    // Padlock: toggles the board camera between Locked (fixed,
                    // default) and Free (WASDQE pan/rotate + scroll zoom).
                    let locked = params.camera_lock.locked;
                    let icon = if locked { "🔒" } else { "🔓" };
                    let btn = egui::Button::new(egui::RichText::new(icon).size(16.0))
                        .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 10))
                        .corner_radius(6.0);
                    if ui
                        .add_sized([32.0, 28.0], btn)
                        .on_hover_text(if locked {
                            "Camera locked — click to free look"
                        } else {
                            "Camera free — click to lock"
                        })
                        .clicked()
                    {
                        params.camera_lock.toggle();
                    }
                });
            });
        });

    if !params.camera_lock.locked {
        egui::Area::new(egui::Id::new("free_camera_hint"))
            .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -12.0])
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(
                        "Free camera unlocked — WASD move, Q/E rotate, scroll to zoom, R to reset",
                    )
                    .size(12.0)
                    .color(egui::Color32::from_rgba_unmultiplied(220, 220, 220, 200)),
                );
            });
    }
}
