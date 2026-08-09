//! Center player bars: rendered directly above and below the board, in both
//! 2D and 3D view modes, replacing the old sidebar-embedded clock/name/
//! capture-tray rows.
//!
//! Data is assembled once per frame in `game_status_ui` (which has full
//! access to `GameUIParams`) and cached in [`PlayerBarsCache`] so the 2D
//! board system (`game_2d::render_2d_board` — a separate Bevy system with
//! its own, unrelated set of params) can read it without every
//! `GameUIParams` field needing to be wired into its own signature.

use crate::rendering::pieces::{PieceColor, PieceType};
use crate::ui::styles::*;
use bevy::prelude::*;
use bevy_egui::egui;

/// Fixed height reserved for each player bar — shared by the 2D board's
/// layout math (`game_2d::render_2d_board`) and the 3D floating `Area`s
/// (`render_center_player_bars_3d`) so the two never drift apart.
pub const PLAYER_BAR_HEIGHT: f32 = 64.0;

/// Everything one player bar needs to render — resolved once per frame.
#[derive(Clone)]
pub struct PlayerBarData {
    pub name: String,
    pub elo: String,
    /// "No moves yet" / last move played / the local player's color.
    pub status_line: String,
    pub clock_secs: Option<f32>,
    pub clock_active: bool,
    pub clock_flagged: bool,
    pub pulse_alpha: u8,
    pub captures: Vec<PieceType>,
    pub is_dark_captures: bool,
    pub capture_delta: i32,
    pub online: Option<bool>,
}

impl PlayerBarData {
    fn empty() -> Self {
        Self {
            name: String::new(),
            elo: String::new(),
            status_line: String::new(),
            clock_secs: None,
            clock_active: false,
            clock_flagged: false,
            pulse_alpha: 0,
            captures: Vec::new(),
            is_dark_captures: false,
            capture_delta: 0,
            online: None,
        }
    }
}

/// Per-frame cache of the top (opponent) / bottom (local) player bar
/// contents — written once by `game_status_ui`, read by both the 3D overlay
/// (same system) and the 2D board system (a separate system later in the
/// same `EguiPrimaryContextPass` chain).
#[derive(Resource)]
pub struct PlayerBarsCache {
    pub top: PlayerBarData,
    pub bottom: PlayerBarData,
}

impl Default for PlayerBarsCache {
    fn default() -> Self {
        Self {
            top: PlayerBarData::empty(),
            bottom: PlayerBarData::empty(),
        }
    }
}

/// Builds the top/bottom player bar data for this frame. Mirrors the
/// identity/clock/capture resolution that used to live directly inside
/// `render_game_right_panel`.
pub fn build_player_bar_data(
    params: &crate::ui::system_params::game_ui::GameUIParams,
) -> (PlayerBarData, PlayerBarData) {
    let local_color = if *params.game_mode == crate::core::GameMode::SinglePlayer {
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
    let is_spectating = *params.game_mode == crate::core::GameMode::Spectator;
    let is_online = matches!(
        *params.game_mode,
        crate::core::GameMode::OnlineMultiplayer | crate::core::GameMode::MultiplayerCompetitive
    );

    let (white_name, white_elo, black_name, black_elo) =
        crate::ui::game::game_ui::resolve_player_names(params, local_color, is_spectating);

    let (top_color, bot_color) = if is_spectating {
        (PieceColor::Black, PieceColor::White)
    } else {
        (opp_color, local_color)
    };
    let (top_name, top_elo) = if top_color == PieceColor::White {
        (white_name.clone(), white_elo.clone())
    } else {
        (black_name.clone(), black_elo.clone())
    };
    let (bot_name, bot_elo) = if bot_color == PieceColor::White {
        (white_name, white_elo)
    } else {
        (black_name, black_elo)
    };

    let white_active = params.current_turn.color == PieceColor::White;
    let top_is_active = (top_color == PieceColor::White) == white_active;
    let bot_is_active = !top_is_active;

    use crate::game::time_control::TimeControl;
    let show_timers = !matches!(params.active_time_control.control, TimeControl::Unlimited);
    let top_time = if top_color == PieceColor::White {
        params.game_timer.white_time_left
    } else {
        params.game_timer.black_time_left
    };
    let bot_time = if bot_color == PieceColor::White {
        params.game_timer.white_time_left
    } else {
        params.game_timer.black_time_left
    };
    let flagged_is = |color: PieceColor| -> bool {
        params.hourglass.flagged_player.as_deref()
            == Some(match color {
                PieceColor::White => "white",
                PieceColor::Black => "black",
            })
    };
    let pulse_alpha =
        ((params.hourglass.elapsed * std::f32::consts::TAU * 2.0).sin() * 87.0 + 168.0) as u8;

    let cap = &*params.game_state.captured;
    let (top_cap, top_is_dark) = if top_color == PieceColor::White {
        (cap.white_captured.clone(), false)
    } else {
        (cap.black_captured.clone(), true)
    };
    let (bot_cap, bot_is_dark) = if bot_color == PieceColor::White {
        (cap.white_captured.clone(), false)
    } else {
        (cap.black_captured.clone(), true)
    };
    let mat_adv = cap.material_advantage();
    let top_delta = if top_color == PieceColor::White {
        mat_adv.max(0)
    } else {
        (-mat_adv).max(0)
    };
    let bot_delta = if bot_color == PieceColor::White {
        mat_adv.max(0)
    } else {
        (-mat_adv).max(0)
    };

    let opponent_online: Option<bool> = if is_online {
        params.p2p_conn.as_ref().map(|c| {
            matches!(
                c.status,
                crate::multiplayer::network::p2p::P2PConnectionStatus::Connected
                    | crate::multiplayer::network::p2p::P2PConnectionStatus::InGame
            )
        })
    } else {
        None
    };

    let top_status = if params.move_history.is_empty() {
        "No moves yet".to_string()
    } else {
        let last_idx = params.move_history.moves.len() - 1;
        params
            .move_history
            .san_at(last_idx)
            .map(|s| format!("Last: {s}"))
            .unwrap_or_else(|| "No moves yet".to_string())
    };
    let bot_status = match bot_color {
        PieceColor::White => "White".to_string(),
        PieceColor::Black => "Black".to_string(),
    };

    let top = PlayerBarData {
        name: top_name,
        elo: top_elo,
        status_line: top_status,
        clock_secs: show_timers.then_some(top_time),
        clock_active: top_is_active,
        clock_flagged: flagged_is(top_color),
        pulse_alpha,
        captures: top_cap,
        is_dark_captures: top_is_dark,
        capture_delta: top_delta,
        online: opponent_online,
    };
    let bottom = PlayerBarData {
        name: bot_name,
        elo: bot_elo,
        status_line: bot_status,
        clock_secs: show_timers.then_some(bot_time),
        clock_active: bot_is_active,
        clock_flagged: flagged_is(bot_color),
        pulse_alpha,
        captures: bot_cap,
        is_dark_captures: bot_is_dark,
        capture_delta: bot_delta,
        online: None,
    };
    (top, bottom)
}

/// Renders one player bar (avatar, name, status/last-move line, captures,
/// clock) — used for both the opponent (top) and local player (bottom),
/// above/below the board, in both 2D and 3D.
pub fn render_player_bar(ui: &mut egui::Ui, data: &PlayerBarData) {
    StyledPanel::sidebar_card()
        .inner_margin(egui::Margin::symmetric(14, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    crate::ui::game::game_ui::render_compact_user_row(
                        ui,
                        &data.name,
                        &data.elo,
                        data.online,
                    );
                    ui.label(
                        egui::RichText::new(&data.status_line)
                            .size(11.5)
                            .color(UiColors::TEXT_SECONDARY),
                    );
                    if !data.captures.is_empty() {
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            crate::ui::game::game_ui::render_captured_pieces_tray(
                                ui,
                                &data.captures,
                                data.is_dark_captures,
                            );
                            if data.capture_delta > 0 {
                                ui.label(
                                    egui::RichText::new(format!("+{}", data.capture_delta))
                                        .size(11.0)
                                        .color(UiColors::TEXT_TERTIARY),
                                );
                            }
                        });
                    }
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(secs) = data.clock_secs {
                        ui.allocate_ui(egui::Vec2::new(104.0, PLAYER_BAR_HEIGHT - 16.0), |ui| {
                            crate::ui::game::game_ui::render_clock_bar(
                                ui,
                                secs,
                                data.clock_active,
                                data.clock_flagged,
                                data.pulse_alpha,
                            );
                        });
                    }
                });
            });
        });
}

/// Floating overlay version of the player bars for 3D mode, where there's no
/// `CentralPanel` to embed them in (the board fills the whole window behind
/// the translucent side panels — see `BoardCamera` in `game/systems/camera.rs`).
/// Mirrors how `render_check_banner`/`render_checkmate_banner` already float
/// over the live 3D board without blocking mesh picking: non-interactable
/// `egui::Area`s, so 3D piece-picking under the board area is unaffected.
pub fn render_center_player_bars_3d(
    ctx: &egui::Context,
    top: &PlayerBarData,
    bottom: &PlayerBarData,
    top_bar_height: f32,
) {
    let bar_width = 440.0;
    egui::Area::new("center_player_bar_top".into())
        .order(egui::Order::Foreground)
        .interactable(false)
        .anchor(
            egui::Align2::CENTER_TOP,
            egui::vec2(0.0, top_bar_height + 12.0),
        )
        .show(ctx, |ui| {
            ui.set_width(bar_width);
            render_player_bar(ui, top);
        });
    egui::Area::new("center_player_bar_bottom".into())
        .order(egui::Order::Foreground)
        .interactable(false)
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -12.0))
        .show(ctx, |ui| {
            ui.set_width(bar_width);
            render_player_bar(ui, bottom);
        });
}
