//! Left sidebar for the in-game screen: game type / rated badge / time
//! control, both players' identity, and (online games only) inline chat.
//!
//! Mirrors the right sidebar's top-opponent / bottom-local-player convention
//! (`crate::ui::game::game_ui::render_game_right_panel`) so the two columns
//! read as one consistent layout, lichess-style.

use crate::core::GameMode;
use crate::rendering::pieces::PieceColor;
use crate::ui::game::game_ui::{render_compact_user_row, resolve_player_names};
use crate::ui::styles::*;
use bevy_egui::egui;

/// Renders the left panel's contents. Called from `game_status_ui` inside an
/// `egui::SidePanel::left`, declared before the central board panel so the
/// board correctly reserves space for it.
pub fn render_game_left_panel(
    ui: &mut egui::Ui,
    params: &mut crate::ui::system_params::game_ui::GameUIParams,
) {
    let local_color = params
        .p2p_conn
        .as_ref()
        .and_then(|c| c.player_color)
        .unwrap_or(PieceColor::White);
    let opp_color = match local_color {
        PieceColor::White => PieceColor::Black,
        PieceColor::Black => PieceColor::White,
    };
    let is_spectating = *params.game_mode == GameMode::Spectator;
    let is_online = matches!(
        *params.game_mode,
        GameMode::OnlineMultiplayer | GameMode::MultiplayerCompetitive
    );

    // ── Game-type card ──────────────────────────────────────────────────────
    StyledPanel::sidebar_card()
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            let tc = params.active_time_control.control;
            let (mode_label, badge): (&str, Option<(&str, egui::Color32)>) = match *params.game_mode
            {
                GameMode::SinglePlayer => ("vs Computer", None),
                GameMode::MultiplayerLocal => ("Local", None),
                GameMode::MultiplayerCompetitive => {
                    ("Online", Some(("Rated", UiColors::ACCENT_GOLD)))
                }
                GameMode::OnlineMultiplayer => {
                    ("Online", Some(("Casual", UiColors::TEXT_SECONDARY)))
                }
                GameMode::Spectator => ("Spectating", None),
                GameMode::PgnReplay => ("Replay", None),
            };

            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(tc.short_label())
                        .size(15.0)
                        .strong()
                        .color(UiColors::TEXT_PRIMARY),
                );
                if let Some((text, color)) = badge {
                    ui.label(egui::RichText::new("•").size(13.0).color(UiColors::TEXT_TERTIARY));
                    ui.label(egui::RichText::new(text).size(12.5).color(color));
                }
                ui.label(egui::RichText::new("•").size(13.0).color(UiColors::TEXT_TERTIARY));
                ui.label(
                    egui::RichText::new(mode_label)
                        .size(12.5)
                        .color(UiColors::TEXT_SECONDARY),
                );
            });

            if params.first_move_deadline.active {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!(
                        "{} seconds to play the first move",
                        params.first_move_deadline.remaining.ceil().max(0.0) as u32
                    ))
                    .size(12.0)
                    .color(UiColors::SUCCESS),
                );
            }
        });

    ui.add_space(6.0);

    // ── Players ─────────────────────────────────────────────────────────────
    let (white_name, white_elo, black_name, black_elo) =
        resolve_player_names(params, local_color, is_spectating);

    let (top_color, bot_color) = if is_spectating {
        (PieceColor::Black, PieceColor::White)
    } else {
        (opp_color, local_color)
    };
    let (top_name, top_elo) = if top_color == PieceColor::White {
        (white_name.as_str(), white_elo.as_str())
    } else {
        (black_name.as_str(), black_elo.as_str())
    };
    let (bot_name, bot_elo) = if bot_color == PieceColor::White {
        (white_name.as_str(), white_elo.as_str())
    } else {
        (black_name.as_str(), black_elo.as_str())
    };

    StyledPanel::sidebar_row()
        .inner_margin(egui::Margin::symmetric(12, 6))
        .show(ui, |ui| {
            render_compact_user_row(ui, top_name, top_elo, None);
        });
    StyledPanel::sidebar_row()
        .inner_margin(egui::Margin::symmetric(12, 6))
        .show(ui, |ui| {
            render_compact_user_row(ui, bot_name, bot_elo, None);
        });

    // ── Chat (online games only) ───────────────────────────────────────────
    if is_online {
        ui.add_space(6.0);
        let remaining_height = ui.available_height() - 8.0;
        StyledPanel::sidebar_card()
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                let player_name = params
                    .player_identity
                    .as_ref()
                    .map(|p| p.display_name().to_string())
                    .unwrap_or_else(|| "me".to_string());
                crate::ui::game::chat_ui::render_chat_section(
                    ui,
                    &mut params.chat_state,
                    &mut params.chat_writer,
                    &player_name,
                    (remaining_height - 90.0).max(80.0),
                );
            });
    }
}
