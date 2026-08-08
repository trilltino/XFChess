//! Left sidebar for the in-game screen: game type / rated badge / time
//! control, both players' ELO, and (online games only) inline chat anchored
//! lower in the panel, below the match-info card.
//!
//! Mirrors the right sidebar's top-opponent / bottom-local-player convention
//! (`crate::ui::game::game_ui::render_game_right_panel`) so the two columns
//! read as one consistent layout, lichess-style.

use crate::core::GameMode;
use crate::rendering::pieces::PieceColor;
use crate::ui::game::game_ui::resolve_player_names;
use crate::ui::styles::*;
use bevy_egui::egui;

/// Fixed height of the chat card — anchored lower in the panel, below the
/// match-info card, rather than growing to fill all remaining space.
const CHAT_CARD_HEIGHT: f32 = 280.0;

/// Renders the left panel's contents. Called from `game_status_ui` inside an
/// `egui::SidePanel::left`, declared before the central board panel so the
/// board correctly reserves space for it.
pub fn render_game_left_panel(
    ui: &mut egui::Ui,
    params: &mut crate::ui::system_params::game_ui::GameUIParams,
) {
    let local_color = if *params.game_mode == GameMode::SinglePlayer {
        match params.ai_params.ai_config.mode.ai_color() {
            PieceColor::White => PieceColor::Black,
            PieceColor::Black => PieceColor::White,
        }
    } else {
        params
            .p2p_conn
            .as_ref()
            .and_then(|c| c.player_color)
            .unwrap_or(PieceColor::White)
    };
    let opp_color = match local_color {
        PieceColor::White => PieceColor::Black,
        PieceColor::Black => PieceColor::White,
    };
    let is_spectating = *params.game_mode == GameMode::Spectator;
    let is_online = matches!(
        *params.game_mode,
        GameMode::OnlineMultiplayer | GameMode::MultiplayerCompetitive
    );

    // Match-info card and moves/controls are both pinned to the top of the
    // panel, just below the top HUD bar — not vertically centered.
    render_match_info_card(ui, params, local_color, opp_color, is_spectating);
    ui.add_space(10.0);
    crate::ui::game::game_ui::render_moves_and_controls(ui, params);

    // ── Chat (online games only), anchored lower in the panel ──────────────
    if is_online {
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            StyledPanel::sidebar_card()
                .fill(UiColors::BG_MID)
                .inner_margin(egui::Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.set_height(CHAT_CARD_HEIGHT);
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
                        (CHAT_CARD_HEIGHT - 90.0).max(80.0),
                    );
                });
        });
    }
}

/// Match-info card: "MATCH" pill, big Rated/Casual headline, a divider, then
/// three stat rows (time control, opponent rating, your rating). Player
/// identity itself (name/avatar/clock) lives in the center player bars above
/// and below the board — see `crate::ui::game::player_bar`.
fn render_match_info_card(
    ui: &mut egui::Ui,
    params: &mut crate::ui::system_params::game_ui::GameUIParams,
    local_color: PieceColor,
    opp_color: PieceColor,
    is_spectating: bool,
) {
    let game_mode = *params.game_mode;
    let tc = params.active_time_control.control;

    // Every non-online mode is inherently unrated; online games show whether
    // they're actually Rated or a Casual online match.
    let headline = match game_mode {
        GameMode::MultiplayerCompetitive => "Rated",
        GameMode::Spectator => "Spectating",
        GameMode::PgnReplay => "Replay",
        _ => "Casual",
    };

    StyledPanel::sidebar_card()
        .fill(UiColors::BG_MID)
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(headline.to_uppercase())
                    .size(TextSize::MD)
                    .family(egui::FontFamily::Name("CinzelBold".into()))
                    .color(UiColors::TEXT_PRIMARY),
            );
            ui.add_space(10.0);
            let sep_w = ui.available_width();
            let (sep_rect, _) =
                ui.allocate_exact_size(egui::Vec2::new(sep_w, 1.0), egui::Sense::hover());
            ui.painter().rect_filled(sep_rect, 0.0, UiColors::BORDER);
            ui.add_space(12.0);

            let (white_elo, black_elo) = {
                let (_, white_elo, _, black_elo) =
                    resolve_player_names(params, local_color, is_spectating);
                (white_elo, black_elo)
            };
            let (top_color, bot_color) = if is_spectating {
                (PieceColor::Black, PieceColor::White)
            } else {
                (opp_color, local_color)
            };
            let top_elo = if top_color == PieceColor::White {
                &white_elo
            } else {
                &black_elo
            };
            let bot_elo = if bot_color == PieceColor::White {
                &white_elo
            } else {
                &black_elo
            };

            let opp_label = if is_spectating {
                "Black Rating"
            } else if game_mode == GameMode::SinglePlayer {
                "Computer Rating"
            } else {
                "Opponent Rating"
            };
            let local_label = if is_spectating {
                "White Rating"
            } else {
                "Your Rating"
            };

            stat_row(ui, &tc.short_label(), "Time Control");
            ui.add_space(10.0);
            stat_row(
                ui,
                if top_elo.is_empty() { "—" } else { top_elo },
                opp_label,
            );
            ui.add_space(10.0);
            stat_row(
                ui,
                if bot_elo.is_empty() { "—" } else { bot_elo },
                local_label,
            );

            if params.first_move_deadline.active {
                ui.add_space(10.0);
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
}
