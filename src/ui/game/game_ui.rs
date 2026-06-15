//! In-game UI for chess game display

/// Classify an opening by its first few moves.
/// Moves encoded as ((from_file, from_rank), (to_file, to_rank)).
pub fn classify_opening(moves: &[((u8, u8), (u8, u8))]) -> Option<&'static str> {
    // Helper: check if `prefix` is a prefix of `moves`.
    let has = |prefix: &[((u8,u8),(u8,u8))]| moves.starts_with(prefix);

    // Coordinate shorthands: (file 0=a..7=h, rank 0=1..7=8)
    let e2e4 = ((4,1),(4,3));  let e7e5 = ((4,6),(4,4));
    let d2d4 = ((3,1),(3,3));  let d7d5 = ((3,6),(3,4));
    let c2c4 = ((2,1),(2,3));  let c7c5 = ((2,6),(2,4));
    let g1f3 = ((6,0),(5,2));  let g8f6 = ((6,7),(5,5));
    let b8c6 = ((1,7),(2,5));
    let f1b5 = ((5,0),(1,4));  let f1c4 = ((5,0),(2,3));
    let _d2d3 = ((3,1),(3,2)); let e7e6 = ((4,6),(4,5));
    let c7c6 = ((2,6),(2,5));  let d7d6 = ((3,6),(3,5));
    let g7g6 = ((6,6),(6,5));  let f2f4 = ((5,1),(5,3));
    let b2b3 = ((1,1),(1,2));  let g2g3 = ((6,1),(6,2));

    // ── 1.e4 lines ──────────────────────────────────────────────────────
    if has(&[e2e4, e7e5]) {
        if has(&[e2e4, e7e5, f2f4])          { return Some("King's Gambit"); }
        if has(&[e2e4, e7e5, g1f3, b8c6, f1b5]) { return Some("Ruy Lopez"); }
        if has(&[e2e4, e7e5, g1f3, b8c6, f1c4]) { return Some("Italian Game"); }
        if has(&[e2e4, e7e5, g1f3, b8c6, d2d4]) { return Some("Scotch Game"); }
        if has(&[e2e4, e7e5, g1f3, g8f6])       { return Some("Russian Game"); }
        if has(&[e2e4, e7e5, g1f3])              { return Some("Open Game"); }
        return Some("Open Game");
    }
    if has(&[e2e4, c7c5]) {
        if has(&[e2e4, c7c5, g1f3, d7d6]) { return Some("Sicilian: Najdorf setup"); }
        if has(&[e2e4, c7c5, g1f3, b8c6]) { return Some("Sicilian: Open"); }
        if has(&[e2e4, c7c5, g1f3, e7e6]) { return Some("Sicilian: Kan/Taimanov"); }
        return Some("Sicilian Defense");
    }
    if has(&[e2e4, e7e6]) { return Some("French Defense"); }
    if has(&[e2e4, c7c6]) {
        if has(&[e2e4, c7c6, d2d4, d7d5]) { return Some("Caro-Kann Defense"); }
        return Some("Caro-Kann Defense");
    }
    if has(&[e2e4, d7d5]) { return Some("Scandinavian Defense"); }
    if has(&[e2e4, g8f6]) { return Some("Alekhine's Defense"); }
    if has(&[e2e4, g7g6]) { return Some("Modern Defense"); }
    if has(&[e2e4, b8c6]) { return Some("Nimzowitsch Defense"); }
    if has(&[e2e4])       { return Some("King's Pawn Opening"); }

    // ── 1.d4 lines ──────────────────────────────────────────────────────
    if has(&[d2d4, d7d5]) {
        if has(&[d2d4, d7d5, c2c4, e7e6])         { return Some("Queen's Gambit Declined"); }
        if has(&[d2d4, d7d5, c2c4, c7c6])         { return Some("Slav Defense"); }
        if has(&[d2d4, d7d5, c2c4])               { return Some("Queen's Gambit"); }
        return Some("Queen's Pawn: Closed");
    }
    if has(&[d2d4, g8f6]) {
        if has(&[d2d4, g8f6, c2c4, g7g6]) { return Some("King's Indian Defense"); }
        if has(&[d2d4, g8f6, c2c4, e7e6]) { return Some("Nimzo/QGD Indian"); }
        if has(&[d2d4, g8f6, c2c4, c7c5]) { return Some("Benoni Defense"); }
        if has(&[d2d4, g8f6, c2c4])       { return Some("Indian Defense"); }
        return Some("Queen's Pawn: Indian setup");
    }
    let f7f5: ((u8,u8),(u8,u8)) = ((5,6),(5,4));
    if has(&[d2d4, f7f5]) { return Some("Dutch Defense"); }
    if has(&[d2d4])                        { return Some("Queen's Pawn Opening"); }

    // ── 1.c4 / 1.Nf3 / others ───────────────────────────────────────────
    if has(&[c2c4]) { return Some("English Opening"); }
    if has(&[g1f3]) { return Some("Réti Opening"); }
    if has(&[g2g3]) { return Some("King's Fianchetto"); }
    if has(&[b2b3]) { return Some("Larsen's Opening"); }

    None
}

use crate::core::GameMode;
use crate::game::components::GamePhase;
use crate::game::resources::system_params::GameStateParams;
use crate::rendering::pieces::PieceColor;
use crate::ui::styles::*;
use crate::ui::system_params::GameUIParams;
use bevy::prelude::*;
use bevy_egui::egui;
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};

/// Flash resource that pulses the +increment label when a player gains time.
#[derive(Resource, Default)]
pub struct IncrementFlash {
    pub elapsed: f32,
    pub active: bool,
    pub color: bool, // true=white gained, false=black gained
}

impl IncrementFlash {
    pub fn trigger(&mut self, white_gained: bool) {
        self.active = true;
        self.elapsed = 0.0;
        self.color = white_gained;
    }

    pub fn tick(&mut self, dt: f32) {
        if self.active {
            self.elapsed += dt;
            if self.elapsed > 0.8 {
                self.active = false;
            }
        }
    }

    pub fn alpha(&self) -> u8 {
        if self.active {
            let t = (self.elapsed / 0.8).min(1.0);
            ((1.0 - t) * 220.0) as u8
        } else {
            0
        }
    }
}

/// Entry in the avatar cache.
pub enum AvatarEntry {
    Loading,
    /// Raw PNG/JPEG bytes received from the background thread, awaiting egui texture creation.
    PendingBytes(Vec<u8>),
    /// Egui texture handle, created once in the rendering system.
    Loaded(egui::TextureHandle),
    Failed,
}

// Safety: egui::TextureHandle is Arc-backed and Send+Sync; Receiver is wrapped in Mutex.
unsafe impl Send for AvatarCache {}
unsafe impl Sync for AvatarCache {}

/// Caches player avatars fetched from the backend.
/// Key: player name / wallet address.
#[derive(Resource)]
pub struct AvatarCache {
    pub entries: HashMap<String, AvatarEntry>,
    rx: std::sync::Mutex<Receiver<(String, Vec<u8>)>>,
    pub tx: Sender<(String, Vec<u8>)>,
}

impl Default for AvatarCache {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self { entries: HashMap::new(), rx: std::sync::Mutex::new(rx), tx }
    }
}

impl AvatarCache {
    /// Kick off a background fetch for `name` if not already in progress.
    pub fn fetch_if_absent(&mut self, name: &str) {
        if self.entries.contains_key(name) { return; }
        self.entries.insert(name.to_string(), AvatarEntry::Loading);
        let tx = self.tx.clone();
        let key = name.to_string();
        let base = crate::multiplayer::network::vps::vps_base();
        std::thread::spawn(move || {
            let url = format!("{}/api/players/{}/avatar", base, key);
            let result = reqwest::blocking::get(&url)
                .and_then(|r| r.bytes().map(|b| b.to_vec()));
            match result {
                Ok(bytes) if !bytes.is_empty() => { let _ = tx.send((key, bytes)); }
                _ => { let _ = tx.send((key, Vec::new())); }
            }
        });
    }

    /// Drain the channel; promotes `Loading` entries to `PendingBytes` or `Failed`.
    pub fn drain_channel(&mut self) {
        let Ok(rx) = self.rx.lock() else { return };
        while let Ok((name, bytes)) = rx.try_recv() {
            if bytes.is_empty() {
                self.entries.insert(name, AvatarEntry::Failed);
            } else {
                self.entries.insert(name, AvatarEntry::PendingBytes(bytes));
            }
        }
    }

    /// Decode any `PendingBytes` entries into egui textures using the current egui context.
    pub fn flush_pending(&mut self, ctx: &egui::Context) {
        for entry in self.entries.values_mut() {
            if let AvatarEntry::PendingBytes(bytes) = entry {
                match image::load_from_memory(bytes) {
                    Ok(dyn_img) => {
                        let rgba = dyn_img.to_rgba8();
                        let (w, h) = rgba.dimensions();
                        let color_img = egui::ColorImage::from_rgba_unmultiplied(
                            [w as usize, h as usize],
                            &rgba,
                        );
                        let tex = ctx.load_texture("player_avatar", color_img,
                            egui::TextureOptions::LINEAR);
                        *entry = AvatarEntry::Loaded(tex);
                    }
                    Err(_) => { *entry = AvatarEntry::Failed; }
                }
            }
        }
    }
}

#[derive(Resource, Default)]
pub struct InGameHudVisibility {
    pub visible: bool,
}

pub fn reset_in_game_hud_visibility(mut hud_visibility: ResMut<InGameHudVisibility>) {
    hud_visibility.visible = true;
}

/// Tracks which player has flagged (run out of time) for the hourglass animation.
#[derive(Resource, Default)]
pub struct TimeoutHourglassState {
    /// Name of the flagged player ("white" or "black"), or None if no flag yet.
    pub flagged_player: Option<String>,
    /// Elapsed seconds since the flag (drives pulsing animation).
    pub elapsed: f32,
}

/// Listens for FlagTimeoutEvent and records the flagged player for the hourglass animation.
pub fn timeout_hourglass_system(
    mut hourglass: ResMut<TimeoutHourglassState>,
    mut flag_reader: bevy::prelude::MessageReader<crate::game::events::FlagTimeoutEvent>,
    time: Res<Time>,
) {
    for ev in flag_reader.read() {
        if hourglass.flagged_player.is_none() {
            hourglass.flagged_player = Some(ev.flagged_player.clone());
            hourglass.elapsed = 0.0;
        }
    }
    if hourglass.flagged_player.is_some() {
        hourglass.elapsed += time.delta_secs();
    }
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

/// Main in-game UI: right sidebar with player info, clocks, move list, and controls.
pub fn game_status_ui(mut params: GameUIParams) {
    if !params.hud_visibility.visible {
        return;
    }

    // PGN replay has its own right panel in replay.rs — skip here.
    if *params.game_mode == GameMode::PgnReplay {
        return;
    }

    // Clone the context so params.contexts is no longer borrowed,
    // allowing &mut params to be passed into SidePanel closures below.
    // egui::Context is Arc-backed, so the clone shares the same frame data.
    let ctx = {
        let Ok(ctx_ref) = params.contexts.ctx_mut() else { return };
        ctx_ref.clone()
    };

    // === CLOCK STATE (for right panel) ===
    use crate::game::time_control::TimeControl;
    let tc = params.active_time_control.control;
    let show_timers = !matches!(tc, TimeControl::Unlimited);

    // Pre-compute clock state needed by the right panel.
    let white_active = params.current_turn.color == PieceColor::White;
    let inc = params.game_timer.increment;
    let white_flagged = params.hourglass.flagged_player.as_deref() == Some("white");
    let black_flagged = params.hourglass.flagged_player.as_deref() == Some("black");
    let hourglass_elapsed = params.hourglass.elapsed;
    let pulse_alpha = ((hourglass_elapsed * std::f32::consts::TAU * 2.0).sin() * 87.0 + 168.0) as u8;

    // Decode any pending avatar bytes into egui textures.
    params.avatar_cache.flush_pending(&ctx);

    // === CHECK/CHECKMATE BANNER ===
    match params.game_state.game_phase.0 {
        GamePhase::Checkmate => render_checkmate_banner(&ctx, &params.game_state),
        GamePhase::Check => render_check_banner(&ctx),
        _ => {}
    }

    // === OPENING NAME ===
    // Show once the first move has been played; hide after move 10 (most openings set by then).
    let move_count = params.move_history.moves.len();
    if move_count >= 1 && move_count <= 20 {
        let seq: Vec<((u8, u8), (u8, u8))> = params.move_history.moves.iter()
            .map(|m| (m.from, m.to))
            .collect();
        if let Some(name) = classify_opening(&seq) {
            egui::Window::new("opening_name")
                .title_bar(false)
                .resizable(false)
                .collapsible(false)
                .anchor(egui::Align2::LEFT_TOP, [12.0, 12.0])
                .frame(
                    egui::Frame::default()
                        .fill(egui::Color32::from_rgba_unmultiplied(20, 20, 28, 200))
                        .corner_radius(6.0)
                        .inner_margin(egui::Margin::symmetric(8, 4)),
                )
                .show(&ctx, |ui| {
                    ui.label(
                        egui::RichText::new(name)
                            .size(11.5)
                            .color(egui::Color32::from_rgb(180, 180, 220)),
                    );
                });
        }
    }

    if params.exit_confirmation.visible && !params.game_state.game_over.is_game_over() {
        let is_online = matches!(
            *params.game_mode,
            GameMode::BraidMultiplayer | GameMode::MultiplayerCompetitive
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
                    ui.horizontal_centered(|ui| {
                        ui.spacing_mut().item_spacing.x = 14.0;
                        if ui.add_sized(
                            [120.0, 40.0],
                            egui::Button::new(egui::RichText::new("No").size(13.0).color(egui::Color32::WHITE))
                                .fill(UiColors::BTN_POPUP_DARK)
                                .stroke(egui::Stroke::NONE)
                                .corner_radius(8.0),
                        ).clicked() {
                            params.exit_confirmation.visible = false;
                            params.exit_confirmation.pending_exit = false;
                        }

                        if ui.add_sized(
                            [120.0, 40.0],
                            egui::Button::new(egui::RichText::new("Yes").size(13.0).color(egui::Color32::WHITE).strong())
                                .fill(UiColors::DANGER)
                                .stroke(egui::Stroke::NONE)
                                .corner_radius(8.0),
                        ).clicked() {
                            params.exit_confirmation.pending_exit = true;
                        }
                    });
                });
            });
    }

    // === RIGHT PANELS ===
    // Solana competitive sidebar declared FIRST → gets rightmost position.
    // game_panel declared SECOND → sits adjacent to board (left of solana_sidebar).

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
            egui::SidePanel::right("solana_sidebar")
                .resizable(true)
                .default_width(250.0)
                .show(&ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(f32::INFINITY)
                        .show(ui, |ui| {
                            if let Some(si) = solana_integration {
                                let mut pv_open = profile_view
                                    .as_ref()
                                    .map(|pv| pv.open)
                                    .unwrap_or(false);
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
                                );
                            }
                        });
                });
        }
    }

    // --- Main game info panel (Lichess-style right sidebar) ---
    egui::SidePanel::right("game_panel")
        .resizable(false)
        .min_width(280.0)
        .frame(
            egui::Frame::default()
                .fill(UiColors::BG_OVERLAY)
                .inner_margin(0.0)
                .stroke(egui::Stroke::NONE),
        )
        .show(&ctx, |ui| {
            render_game_right_panel(
                ui,
                &mut params,
                show_timers,
                white_active,
                white_flagged,
                black_flagged,
                pulse_alpha,
                inc,
            );
        });
}

// ── Lichess-style right panel helpers ────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn render_game_right_panel(
    ui: &mut egui::Ui,
    params: &mut crate::ui::system_params::game_ui::GameUIParams,
    show_timers: bool,
    white_active: bool,
    white_flagged: bool,
    black_flagged: bool,
    pulse_alpha: u8,
    increment: f32,
) {
    use crate::game::resources::TurnPhase;

    let local_color = params.p2p_conn.as_ref()
        .and_then(|c| c.player_color)
        .unwrap_or(PieceColor::White);
    let opp_color = match local_color {
        PieceColor::White => PieceColor::Black,
        PieceColor::Black => PieceColor::White,
    };
    let is_spectating = *params.game_mode == crate::core::GameMode::Spectator;
    let is_online = matches!(
        *params.game_mode,
        crate::core::GameMode::BraidMultiplayer | crate::core::GameMode::MultiplayerCompetitive
    );

    // Build name/elo for white and black from available sources.
    let white_name: String;
    let white_elo: String;
    let black_name: String;
    let black_elo: String;

    if is_spectating {
        let w = params.spectator_mode.white_player.as_ref();
        let b = params.spectator_mode.black_player.as_ref();
        white_name = w.map(|p| p.username.clone()).unwrap_or_else(|| "White".to_string());
        white_elo  = w.map(|p| format!("{}", p.rating)).unwrap_or_default();
        black_name = b.map(|p| p.username.clone()).unwrap_or_else(|| "Black".to_string());
        black_elo  = b.map(|p| format!("{}", p.rating)).unwrap_or_default();
    } else {
        #[cfg(feature = "solana")]
        {
            if let (Some(ref profile), Some(ref comp)) =
                (params.solana_profile.as_ref(), params.competitive_match.as_ref())
            {
                let is_white_local = local_color == PieceColor::White;
                if is_white_local {
                    white_name = profile.username.clone();
                    white_elo  = format!("{}", profile.elo);
                    black_name = comp.opponent_username.clone();
                    black_elo  = format!("{}", comp.opponent_elo);
                } else {
                    black_name = profile.username.clone();
                    black_elo  = format!("{}", profile.elo);
                    white_name = comp.opponent_username.clone();
                    white_elo  = format!("{}", comp.opponent_elo);
                }
            } else {
                white_name = params.players.player_1.name.clone();
                white_elo  = String::new();
                black_name = params.players.player_2.name.clone();
                black_elo  = String::new();
            }
        }
        #[cfg(not(feature = "solana"))]
        {
            white_name = params.players.player_1.name.clone();
            white_elo  = String::new();
            black_name = params.players.player_2.name.clone();
            black_elo  = String::new();
        }
    }

    // Determine top (opponent) / bottom (local) layout.
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

    let top_is_active = top_color == PieceColor::White && white_active
        || top_color == PieceColor::Black && !white_active;
    let top_flagged = if top_color == PieceColor::White { white_flagged } else { black_flagged };
    let bot_flagged = if bot_color == PieceColor::White { white_flagged } else { black_flagged };

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

    let cap = &*params.game_state.captured;
    // white_captured = dark pieces taken by white; black_captured = light pieces taken by black.
    let (top_cap, top_is_dark) = if top_color == PieceColor::White {
        (cap.white_captured.as_slice(), false)
    } else {
        (cap.black_captured.as_slice(), true)
    };
    let (bot_cap, bot_is_dark) = if bot_color == PieceColor::White {
        (cap.white_captured.as_slice(), false)
    } else {
        (cap.black_captured.as_slice(), true)
    };
    let mat_adv = cap.material_advantage();
    let top_delta = if top_color == PieceColor::White { mat_adv.max(0) } else { (-mat_adv).max(0) };
    let bot_delta = if bot_color == PieceColor::White { mat_adv.max(0) } else { (-mat_adv).max(0) };

    let opponent_online: Option<bool> = if is_online {
        params.p2p_conn.as_ref().map(|c| matches!(
            c.status,
            crate::multiplayer::network::p2p::P2PConnectionStatus::Connected
                | crate::multiplayer::network::p2p::P2PConnectionStatus::InGame
        ))
    } else {
        None
    };

    let p = egui::Margin::symmetric(12, 6);
    let bot_is_active = !top_is_active;

    // ── OPPONENT (top of panel) ───────────────────────────────────────────────
    // material tray
    if !top_cap.is_empty() || top_delta > 0 {
        egui::Frame::none().inner_margin(egui::Margin::symmetric(12, 4)).show(ui, |ui| {
            ui.horizontal(|ui| {
                render_captured_pieces_tray(ui, top_cap, top_is_dark);
                if top_delta > 0 {
                    ui.label(egui::RichText::new(format!("+{}", top_delta))
                        .size(11.0).color(UiColors::TEXT_TERTIARY));
                }
            });
        });
    }
    // clock
    if show_timers {
        render_clock_bar(ui, top_time, top_is_active, top_flagged, pulse_alpha, increment, &params.increment_flash);
    }
    // name row
    egui::Frame::none().inner_margin(p).show(ui, |ui| {
        render_compact_user_row(ui, top_name, top_elo, opponent_online);
    });

    ui.add_space(6.0);

    // ── MOVE LIST ─────────────────────────────────────────────────────────────
    // Estimate reserved height: name row (~28) + clock (~62) + mat (~16) + controls (~70) + spacing × 3
    let clock_h = if show_timers { 62.0 * 2.0 } else { 0.0 };
    let reserved = 28.0 * 2.0 + clock_h + 16.0 * 2.0 + 80.0 + 30.0;
    let move_height = (ui.available_height() - reserved).max(80.0);

    egui::Frame::none()
        .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 40))
        .inner_margin(egui::Margin::symmetric(0, 0))
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("game_moves_scroll")
                .max_height(move_height)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    egui::Frame::none()
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            render_move_list_paired(ui, &params.move_history, &params.eval_history);
                        });
                });
        });

    ui.add_space(4.0);

    // ── CONTROLS ─────────────────────────────────────────────────────────────
    egui::Frame::none()
        .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 30))
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            let is_game_over = params.game_state.game_over.is_game_over();
            let is_waiting   = params.turn_ctx.phase == TurnPhase::WaitingForInput;

            if !is_game_over {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    // Resign — red text button
                    if ui.add(
                        egui::Button::new(
                            egui::RichText::new("✕  Resign").size(12.0)
                                .color(egui::Color32::from_rgb(220, 80, 80)),
                        )
                        .fill(egui::Color32::from_rgba_unmultiplied(60, 18, 18, 180))
                        .min_size(egui::Vec2::new(90.0, 28.0)),
                    ).clicked() && is_waiting {
                        let winner = match params.current_turn.color {
                            PieceColor::White => "black".to_string(),
                            PieceColor::Black => "white".to_string(),
                        };
                        params.resign_writer.write(crate::game::events::ResignEvent { winner, remote: false });
                    }

                    if is_online {
                        let draw_offered = params.pending_draw.from_player.is_some();
                        if ui.add(
                            egui::Button::new(
                                egui::RichText::new(if draw_offered { "½  Sent" } else { "½  Draw" })
                                    .size(12.0)
                                    .color(egui::Color32::from_rgb(180, 180, 80)),
                            )
                            .fill(egui::Color32::from_rgba_unmultiplied(45, 45, 12, 180))
                            .min_size(egui::Vec2::new(80.0, 28.0))
                            .sense(if draw_offered { egui::Sense::hover() } else { egui::Sense::click() }),
                        ).clicked() && !draw_offered {
                            let player = match params.current_turn.color {
                                PieceColor::White => "white".to_string(),
                                PieceColor::Black => "black".to_string(),
                            };
                            params.draw_writer.write(crate::game::events::DrawOfferEvent { player, remote: false });
                        }
                    }
                });
                ui.add_space(6.0);
            }

            // View toggle
            let view_label = match params.view_preferences.local_view {
                crate::game::view_mode::ViewMode::Standard3D => "⬡  2D View",
                _ => "⬡  3D View",
            };
            if ui.add(
                egui::Button::new(egui::RichText::new(view_label).size(12.0).color(egui::Color32::from_gray(180)))
                    .fill(egui::Color32::from_rgba_unmultiplied(40, 40, 55, 180))
                    .min_size(egui::Vec2::new(90.0, 26.0)),
            ).clicked() {
                params.view_preferences.toggle_view();
                *params.view_mode = params.view_preferences.local_view;
            }
        });

    ui.add_space(4.0);

    // ── LOCAL PLAYER (bottom of panel) ───────────────────────────────────────
    // name row
    egui::Frame::none().inner_margin(p).show(ui, |ui| {
        render_compact_user_row(ui, bot_name, bot_elo, None);
    });
    // clock
    if show_timers {
        render_clock_bar(ui, bot_time, bot_is_active, bot_flagged, pulse_alpha, increment, &params.increment_flash);
    }
    // material tray
    if !bot_cap.is_empty() || bot_delta > 0 {
        egui::Frame::none().inner_margin(egui::Margin::symmetric(12, 4)).show(ui, |ui| {
            ui.horizontal(|ui| {
                render_captured_pieces_tray(ui, bot_cap, bot_is_dark);
                if bot_delta > 0 {
                    ui.label(egui::RichText::new(format!("+{}", bot_delta))
                        .size(11.0).color(UiColors::TEXT_TERTIARY));
                }
            });
        });
    }
}

fn render_compact_user_row(
    ui: &mut egui::Ui,
    name: &str,
    elo: &str,
    online: Option<bool>,
) {
    let display_name = if name.is_empty() { "Player" } else { name };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        // Online indicator dot via unicode — no allocate/painter needed
        if let Some(is_online) = online {
            let (dot, col) = if is_online {
                ("●", egui::Color32::from_rgb(80, 200, 80))
            } else {
                ("●", egui::Color32::from_gray(70))
            };
            ui.label(egui::RichText::new(dot).size(9.0).color(col));
        }
        // Identicon badge via Frame — avoids allocate+painter cursor bug
        let ic_color = identicon_color(display_name);
        let initial = display_name.chars().next().unwrap_or('?').to_uppercase().next().unwrap_or('?');
        egui::Frame::none()
            .fill(ic_color)
            .corner_radius(12.0)
            .inner_margin(egui::Margin::symmetric(5, 2))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(initial.to_string())
                        .size(11.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
            });
        // Name
        ui.label(egui::RichText::new(display_name).size(13.0).color(UiColors::TEXT_PRIMARY).strong());
        // ELO — right-aligned if present
        if !elo.is_empty() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(elo).size(11.0).color(UiColors::TEXT_TERTIARY));
            });
        }
    });
}

fn render_clock_bar(
    ui: &mut egui::Ui,
    time_secs: f32,
    is_active: bool,
    flagged: bool,
    pulse_alpha: u8,
    _increment: f32,
    _increment_flash: &IncrementFlash,
) {
    let low_time = time_secs < 10.0;
    let bg_fill = if is_active && low_time {
        egui::Color32::from_rgba_unmultiplied(130, 18, 18, 230)
    } else if is_active {
        egui::Color32::from_rgba_unmultiplied(30, 55, 42, 220)
    } else {
        egui::Color32::from_rgba_unmultiplied(22, 22, 28, 200)
    };
    let (font_size, color) = if is_active {
        let c = if low_time {
            egui::Color32::from_rgb(255, 90, 90)
        } else {
            egui::Color32::from_gray(240)
        };
        (38.0_f32, c)
    } else {
        (24.0_f32, egui::Color32::from_gray(100))
    };

    egui::Frame::default()
        .fill(bg_fill)
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.vertical_centered(|ui| {
                let time_str = if flagged {
                    format!("⧖ {}", format_time(time_secs))
                } else {
                    format_time(time_secs)
                };
                let text_color = if flagged {
                    egui::Color32::from_rgba_unmultiplied(255, 130, 0, pulse_alpha)
                } else {
                    color
                };
                ui.label(
                    egui::RichText::new(time_str)
                        .size(font_size)
                        .strong()
                        .color(text_color),
                );
            });
        });
}

fn render_move_list_paired(
    ui: &mut egui::Ui,
    history: &crate::game::resources::history::MoveHistory,
    eval_history: &crate::ui::game::game_2d::EvalHistory,
) {
    if history.is_empty() {
        ui.label(
            egui::RichText::new("No moves yet")
                .size(12.0)
                .color(UiColors::TEXT_TERTIARY),
        );
        return;
    }
    let moves = &history.moves;
    let total = moves.len();
    egui::Grid::new("move_list_grid")
        .num_columns(3)
        .min_col_width(28.0)
        .spacing([2.0, 1.0])
        .show(ui, |ui| {
            for move_num in 1..=((total + 1) / 2) {
                let white_idx = (move_num - 1) * 2;
                let black_idx = white_idx + 1;
                ui.label(
                    egui::RichText::new(format!("{}.", move_num))
                        .size(11.0)
                        .color(UiColors::TEXT_TERTIARY),
                );
                if white_idx < total {
                    let mv = &moves[white_idx];
                    let mut text = format_move_algebraic(mv);
                    if let Some((sym, _)) = annotate_move(white_idx, PieceColor::White, &eval_history.scores) {
                        text.push(' ');
                        text.push_str(sym);
                    }
                    ui.label(egui::RichText::new(text).size(13.0).color(UiColors::TEXT_PRIMARY).strong());
                } else {
                    ui.label("");
                }
                if black_idx < total {
                    let mv = &moves[black_idx];
                    let mut text = format_move_algebraic(mv);
                    if let Some((sym, _)) = annotate_move(black_idx, PieceColor::Black, &eval_history.scores) {
                        text.push(' ');
                        text.push_str(sym);
                    }
                    ui.label(egui::RichText::new(text).size(13.0).color(UiColors::TEXT_SECONDARY).strong());
                } else if white_idx < total {
                    ui.label(egui::RichText::new("…").size(13.0).color(UiColors::TEXT_TERTIARY));
                } else {
                    ui.label("");
                }
                ui.end_row();
            }
        });
}

// ── end Lichess panel helpers ─────────────────────────────────────────────────

/// Render player information section
/// Derive a stable background color from a player name for the identicon badge.
/// Uses FNV-1a hash to map the name to one of 8 pleasant hues.
pub fn identicon_color(name: &str) -> egui::Color32 {
    let mut hash: u32 = 2166136261;
    for b in name.bytes() {
        hash ^= b as u32;
        hash = hash.wrapping_mul(16777619);
    }
    // Map to 8 hue buckets (HSL-like, pre-defined colors)
    let colors = [
        egui::Color32::from_rgb(70,  130, 200), // blue
        egui::Color32::from_rgb(60,  160, 100), // green
        egui::Color32::from_rgb(200,  90,  60), // red-orange
        egui::Color32::from_rgb(140,  80, 200), // purple
        egui::Color32::from_rgb(200, 150,  40), // amber
        egui::Color32::from_rgb(40,  170, 170), // teal
        egui::Color32::from_rgb(200,  70, 140), // pink
        egui::Color32::from_rgb(100, 100, 120), // slate
    ];
    colors[(hash as usize) % colors.len()]
}

/// Render captured piece symbols as a compact tray row.
/// `pieces` = pieces captured BY this side (the opponent's piece type).
/// `is_dark` true = render as dark pieces (captured by white), false = light pieces (captured by black).
fn render_captured_pieces_tray(ui: &mut egui::Ui, pieces: &[crate::rendering::pieces::PieceType], is_dark: bool) {
    use crate::rendering::pieces::PieceType;
    if pieces.is_empty() { return; }

    let (sym_color, _bg) = if is_dark {
        (egui::Color32::from_gray(30), egui::Color32::from_rgba_unmultiplied(230, 220, 195, 60))
    } else {
        (egui::Color32::from_gray(225), egui::Color32::from_rgba_unmultiplied(50, 50, 60, 60))
    };

    let mut sorted = pieces.to_vec();
    let order = |p: &PieceType| match p {
        PieceType::Queen  => 0,
        PieceType::Rook   => 1,
        PieceType::Bishop => 2,
        PieceType::Knight => 3,
        PieceType::Pawn   => 4,
        PieceType::King   => 5,
    };
    sorted.sort_by_key(order);

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for pt in &sorted {
            let sym = if is_dark {
                match pt {
                    PieceType::Queen  => "♛",
                    PieceType::Rook   => "♜",
                    PieceType::Bishop => "♝",
                    PieceType::Knight => "♞",
                    PieceType::Pawn   => "♟",
                    PieceType::King   => "♚",
                }
            } else {
                match pt {
                    PieceType::Queen  => "♕",
                    PieceType::Rook   => "♖",
                    PieceType::Bishop => "♗",
                    PieceType::Knight => "♘",
                    PieceType::Pawn   => "♙",
                    PieceType::King   => "♔",
                }
            };
            ui.label(egui::RichText::new(sym).size(12.0).color(sym_color));
        }
    });
}

fn annotate_move(ply_idx: usize, color: PieceColor, scores: &[i16]) -> Option<(&'static str, egui::Color32)> {
    use crate::ui::game::game_2d::MoveQuality;
    if scores.len() <= ply_idx { return None; }
    let after  = scores[ply_idx];
    let before = if ply_idx > 0 { scores[ply_idx - 1] } else { 0 };
    let gain   = if color == PieceColor::White { after - before } else { before - after };
    MoveQuality::classify(gain).map(|q| (q.symbol(), q.color()))
}

/// Format a move record as algebraic notation
fn format_move_algebraic(mv: &crate::game::components::MoveRecord) -> String {
    use crate::rendering::pieces::PieceType;
    
    // Piece letter (or empty for pawn)
    let piece_letter = match mv.piece_type {
        PieceType::King => "K",
        PieceType::Queen => "Q",
        PieceType::Rook => "R",
        PieceType::Bishop => "B",
        PieceType::Knight => "N",
        PieceType::Pawn => "",
    };
    
    // Destination square
    let from_file = (b'a' + mv.from.0) as char;
    let _from_rank = mv.from.1 + 1;
    let to_file = (b'a' + mv.to.0) as char;
    let to_rank = mv.to.1 + 1;
    
    // Build notation
    let mut notation = String::new();
    
    // Castling
    if mv.is_castling {
        if mv.to.0 > mv.from.0 {
            notation = "O-O".to_string();
        } else {
            notation = "O-O-O".to_string();
        }
    } else {
        // Normal move
        if mv.piece_type == PieceType::Pawn && mv.captured.is_some() {
            // Pawn capture includes file
            notation.push(from_file);
        } else {
            notation.push_str(piece_letter);
        }
        
        // Capture symbol
        if mv.captured.is_some() {
            notation.push('x');
        }
        
        // Destination
        notation.push(to_file);
        notation.push_str(&to_rank.to_string());
    }
    
    // Check/Checkmate
    if mv.is_checkmate {
        notation.push('#');
    } else if mv.is_check {
        notation.push('+');
    }
    
    notation
}

/// Render a sleek "CHECK" indicator at the top of the screen
fn render_check_banner(ctx: &egui::Context) {
    egui::Window::new("check_banner")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_TOP, [0.0, 16.0]) // Slightly higher position
        .frame(
            egui::Frame::default()
                .fill(egui::Color32::from_rgba_unmultiplied(173, 92, 47, 200)) // Primary bronze with transparency
                .corner_radius(20.0) // Pill shape
                .inner_margin(egui::Margin::symmetric(16, 8)) // Compact padding
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(244, 187, 68, 150))), // Gold accent border
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                // Crown icon for check indication
                ui.label(
                    egui::RichText::new("")
                        .size(16.0)
                        .color(egui::Color32::from_rgb(244, 187, 68)), // Gold color
                );
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new("CHECK")
                        .size(13.0)
                        .color(egui::Color32::WHITE)
                        .strong()
                        .extra_letter_spacing(1.0),
                );
            });
        });
}

/// Render a minimal checkmate pill — small, unobtrusive, top-center.
fn render_checkmate_banner(ctx: &egui::Context, game_state: &GameStateParams) {
    let (winner_text, _) = match game_state.game_over.winner() {
        Some(PieceColor::White) => ("White wins", egui::Color32::from_rgb(240, 240, 240)),
        Some(PieceColor::Black) => ("Black wins", egui::Color32::from_rgb(200, 200, 200)),
        None                    => ("Draw",        egui::Color32::from_rgb(200, 200, 200)),
    };

    let label = format!("Checkmate  \u{2022}  {}", winner_text);

    egui::Window::new("checkmate_banner")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_TOP, [0.0, 16.0])
        .frame(
            egui::Frame::default()
                .fill(egui::Color32::from_rgba_unmultiplied(12, 12, 14, 210))
                .corner_radius(20.0)
                .inner_margin(egui::Margin::symmetric(18, 8))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30))),
        )
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new(label)
                    .size(13.0)
                    .color(egui::Color32::from_rgb(210, 210, 210)),
            );
        });
}

/// Format time in seconds to MM:SS format
fn format_time(seconds: f32) -> String {
    let total_seconds = seconds.max(0.0) as u32;
    let minutes = total_seconds / 60;
    let secs = total_seconds % 60;
    format!("{:02}:{:02}", minutes, secs)
}

/// Overlay system: shows an Accept/Decline banner when the opponent has offered a draw.
/// Fires [`DrawResponseEvent`] (remote=false) on click so the network layer forwards it.
pub fn draw_offer_ui(
    mut contexts: bevy_egui::EguiContexts,
    pending: Res<crate::game::systems::network_move::PendingDrawOffer>,
    mut draw_response: bevy::prelude::MessageWriter<crate::game::events::DrawResponseEvent>,
    p2p_conn: Option<Res<crate::multiplayer::network::p2p::P2PConnectionState>>,
) {
    let Some(from) = pending.from_player.as_ref() else { return };
    let Ok(ctx) = contexts.ctx_mut() else { return };

    egui::Window::new("draw_offer_banner")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_TOP, [0.0, 80.0])
        .fixed_size([340.0, 130.0])
        .frame(StyledPanel::popup())
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(TextStyle::popup_title(format!("{} offers a draw", from)));
                ui.add_space(12.0);
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;

                    let local_player = p2p_conn
                        .as_ref()
                        .and_then(|c| c.player_color)
                        .map(|col| match col {
                            crate::rendering::pieces::PieceColor::White => "white",
                            crate::rendering::pieces::PieceColor::Black => "black",
                        })
                        .unwrap_or("white")
                        .to_string();

                    if ui.add_sized(
                        [120.0, 38.0],
                        egui::Button::new(egui::RichText::new("Accept").size(13.0).color(egui::Color32::WHITE).strong())
                            .fill(egui::Color32::from_rgb(34, 100, 50))
                            .stroke(egui::Stroke::NONE)
                            .corner_radius(8.0),
                    ).clicked() {
                        draw_response.write(crate::game::events::DrawResponseEvent {
                            player: local_player.clone(),
                            accepted: true,
                            remote: false,
                        });
                    }

                    if ui.add_sized(
                        [120.0, 38.0],
                        egui::Button::new(egui::RichText::new("Decline").size(13.0).color(egui::Color32::WHITE))
                            .fill(UiColors::BTN_POPUP_DARK)
                            .stroke(egui::Stroke::NONE)
                            .corner_radius(8.0),
                    ).clicked() {
                        draw_response.write(crate::game::events::DrawResponseEvent {
                            player: local_player,
                            accepted: false,
                            remote: false,
                        });
                    }
                });
            });
        });
}

/// Pause/resume button for online multiplayer: pauses/resumes both clocks.
/// Shown only in BraidMultiplayer mode and only when the game is active.
pub fn pause_resume_ui(
    mut contexts: bevy_egui::EguiContexts,
    mut game_timer: ResMut<crate::game::resources::GameTimer>,
    game_over: Res<crate::game::resources::GameOverState>,
    game_mode: Res<crate::core::GameMode>,
    network_state: Option<Res<crate::multiplayer::BraidNetworkState>>,
    session: Option<Res<crate::multiplayer::network::braid_pvp::BraidPvpSession>>,
    p2p_conn: Option<Res<crate::multiplayer::network::p2p::P2PConnectionState>>,
) {
    use crate::core::GameMode;
    use crate::multiplayer::network::protocol::NetworkMessage;

    if !matches!(*game_mode, GameMode::BraidMultiplayer | GameMode::MultiplayerCompetitive) { return; }
    if game_over.is_game_over() { return; }
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let is_paused = !game_timer.is_running;
    let label = if is_paused { "▶ Resume" } else { "⏸ Pause" };
    let color = if is_paused {
        egui::Color32::from_rgb(40, 160, 80)
    } else {
        egui::Color32::from_rgb(160, 120, 40)
    };

    egui::Window::new("pause_resume_btn")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::RIGHT_BOTTOM, [-20.0, -60.0])
        .fixed_size([100.0, 36.0])
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            if ui.add_sized([100.0, 34.0],
                egui::Button::new(egui::RichText::new(label).size(13.0).color(egui::Color32::WHITE).strong())
                    .fill(color).corner_radius(6.0),
            ).clicked() {
                let game_id = session.as_ref()
                    .and_then(|s| s.game_id.parse::<u64>().ok())
                    .unwrap_or(0);
                let player = p2p_conn.as_ref()
                    .and_then(|c| c.player_color)
                    .map(|col| match col {
                        PieceColor::White => "white",
                        PieceColor::Black => "black",
                    })
                    .unwrap_or("white")
                    .to_string();

                if is_paused {
                    game_timer.is_running = true;
                    if let Some(ref ns) = network_state {
                        if let Some(ref tx) = ns.message_sender {
                            let _ = tx.send(NetworkMessage::ResumeRequest { game_id, player });
                        }
                    }
                } else {
                    game_timer.is_running = false;
                    if let Some(ref ns) = network_state {
                        if let Some(ref tx) = ns.message_sender {
                            let _ = tx.send(NetworkMessage::PauseRequest { game_id, player });
                        }
                    }
                }
            }
        });
}

/// Overlay system: shows an Accept/Decline banner when the opponent has offered a rematch.
pub fn rematch_offer_ui(
    mut contexts: bevy_egui::EguiContexts,
    mut pending: ResMut<crate::game::systems::network_move::PendingRematchOffer>,
    mut rematch_response: bevy::prelude::MessageWriter<crate::game::events::RematchResponseEvent>,
    p2p_conn: Option<Res<crate::multiplayer::network::p2p::P2PConnectionState>>,
) {
    let Some(from) = pending.from_player.clone() else { return };
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let local_player = p2p_conn
        .as_ref()
        .and_then(|c| c.player_color)
        .map(|col| match col {
            crate::rendering::pieces::PieceColor::White => "white",
            crate::rendering::pieces::PieceColor::Black => "black",
        })
        .unwrap_or("white")
        .to_string();

    egui::Window::new("rematch_offer_banner")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_TOP, [0.0, 80.0])
        .fixed_size([340.0, 130.0])
        .frame(StyledPanel::popup())
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(TextStyle::popup_title(format!("{} offers a rematch", from)));
                ui.add_space(12.0);
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;

                    if ui.add_sized([120.0, 38.0],
                        egui::Button::new(egui::RichText::new("Accept").size(13.0).color(egui::Color32::WHITE).strong())
                            .fill(egui::Color32::from_rgb(34, 100, 50))
                            .stroke(egui::Stroke::NONE)
                            .corner_radius(8.0),
                    ).clicked() {
                        rematch_response.write(crate::game::events::RematchResponseEvent {
                            player: local_player.clone(),
                            accepted: true,
                            remote: false,
                        });
                        pending.from_player = None;
                    }

                    if ui.add_sized([120.0, 38.0],
                        egui::Button::new(egui::RichText::new("Decline").size(13.0).color(egui::Color32::WHITE))
                            .fill(UiColors::BTN_POPUP_DARK)
                            .stroke(egui::Stroke::NONE)
                            .corner_radius(8.0),
                    ).clicked() {
                        rematch_response.write(crate::game::events::RematchResponseEvent {
                            player: local_player,
                            accepted: false,
                            remote: false,
                        });
                        pending.from_player = None;
                    }
                });
            });
        });
}

/// Post-game evaluation overlay: shown when the game is over with result, reason, and rematch button.
pub fn post_game_overlay(
    mut contexts: bevy_egui::EguiContexts,
    game_over: Res<crate::game::resources::GameOverState>,
    game_mode: Res<crate::core::GameMode>,
    mut next_state: ResMut<NextState<crate::core::GameState>>,
    mut rematch_offer: bevy::prelude::MessageWriter<crate::game::events::RematchOfferEvent>,
    p2p_conn: Option<Res<crate::multiplayer::network::p2p::P2PConnectionState>>,
    move_history: Res<crate::game::resources::MoveHistory>,
) {
    if !game_over.is_game_over() { return; }
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let is_online = matches!(*game_mode, crate::core::GameMode::BraidMultiplayer | crate::core::GameMode::MultiplayerCompetitive);

    let result_line = game_over.message();
    let reason_line = match *game_over {
        crate::game::resources::GameOverState::WhiteWon => "by Checkmate",
        crate::game::resources::GameOverState::BlackWon => "by Checkmate",
        crate::game::resources::GameOverState::WhiteWonByResignation => "by Resignation",
        crate::game::resources::GameOverState::BlackWonByResignation => "by Resignation",
        crate::game::resources::GameOverState::WhiteWonByTime => "on Time",
        crate::game::resources::GameOverState::BlackWonByTime => "on Time",
        crate::game::resources::GameOverState::Stalemate => "Stalemate / Draw",
        _ => "",
    };

    let total_moves = move_history.moves.len();

    egui::Window::new("post_game_overlay")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size([360.0, 280.0])
        .frame(StyledPanel::popup())
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                // Result header
                let (result_color, icon) = match game_over.winner() {
                    Some(PieceColor::White) => (egui::Color32::from_rgb(220, 220, 220), "♔"),
                    Some(PieceColor::Black) => (egui::Color32::from_rgb(180, 140, 255), "♚"),
                    None => (egui::Color32::GOLD, "="),
                };
                ui.label(egui::RichText::new(icon).size(36.0).color(result_color));
                ui.add_space(4.0);
                ui.label(egui::RichText::new(result_line)
                    .size(22.0)
                    .family(egui::FontFamily::Name("CinzelBold".into()))
                    .color(result_color));
                if !reason_line.is_empty() {
                    ui.label(TextStyle::popup_body(reason_line));
                }
                ui.add_space(8.0);
                ui.label(TextStyle::popup_body(format!("{} moves played", total_moves)));

                ui.add_space(20.0);
                ui.separator();
                ui.add_space(12.0);

                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;

                    // Rematch button (online only)
                    if is_online {
                        let local_player = p2p_conn.as_ref()
                            .and_then(|c| c.player_color)
                            .map(|col| match col {
                                PieceColor::White => "white",
                                PieceColor::Black => "black",
                            })
                            .unwrap_or("white")
                            .to_string();

                        if ui.add_sized([120.0, 40.0],
                            egui::Button::new(egui::RichText::new("Rematch").size(13.0).color(egui::Color32::WHITE).strong())
                                .fill(egui::Color32::from_rgb(34, 80, 160))
                                .stroke(egui::Stroke::NONE)
                                .corner_radius(8.0),
                        ).clicked() {
                            rematch_offer.write(crate::game::events::RematchOfferEvent {
                                player: local_player,
                                remote: false,
                            });
                        }
                    }

                    if ui.add_sized([120.0, 40.0],
                        egui::Button::new(egui::RichText::new("Main Menu").size(13.0).color(egui::Color32::WHITE))
                            .fill(UiColors::BTN_POPUP_DARK)
                            .stroke(egui::Stroke::NONE)
                            .corner_radius(8.0),
                    ).clicked() {
                        next_state.set(crate::core::GameState::MainMenu);
                    }
                });
            });
        });
}

// ── Opponent disconnect popup ──────────────────────────────────────────────────

/// Resource tracking opponent disconnect state.
#[derive(Resource, Default)]
pub struct OpponentDisconnectState {
    /// When the opponent last dropped their connection.
    pub disconnected_at: Option<std::time::Instant>,
    /// Whether we have already fired `FlagTimeoutEvent` for this disconnect.
    pub timed_out: bool,
}

impl OpponentDisconnectState {
    pub fn elapsed_secs(&self) -> u64 {
        self.disconnected_at.map(|t| t.elapsed().as_secs()).unwrap_or(0)
    }
    pub fn remaining_secs(&self) -> u64 {
        60u64.saturating_sub(self.elapsed_secs())
    }
}

/// Renders a small ping chip (colored dot + Nms label) when in an online game.
pub fn ping_chip_ui(
    mut contexts: bevy_egui::EguiContexts,
    p2p_conn: Option<Res<crate::multiplayer::network::p2p::P2PConnectionState>>,
    game_mode: Res<crate::core::GameMode>,
) {
    use crate::core::GameMode;
    if !matches!(*game_mode, GameMode::BraidMultiplayer | GameMode::MultiplayerCompetitive) {
        return;
    }
    let Some(conn) = p2p_conn else { return };
    let Some(rtt) = conn.last_rtt_ms else { return };
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let (dot_color, label) = if rtt < 80 {
        (egui::Color32::from_rgb(60, 200, 80), format!("{rtt}ms"))
    } else if rtt < 200 {
        (egui::Color32::from_rgb(230, 180, 40), format!("{rtt}ms"))
    } else {
        (egui::Color32::from_rgb(220, 70, 70), format!("{rtt}ms"))
    };

    egui::Window::new("ping_chip")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::RIGHT_BOTTOM, [-20.0, -20.0])
        .frame(
            egui::Frame::default()
                .fill(egui::Color32::from_rgba_unmultiplied(18, 18, 24, 210))
                .corner_radius(12.0)
                .inner_margin(egui::Margin::symmetric(8, 4)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                let (dot_rect, _) = ui.allocate_exact_size(egui::Vec2::splat(10.0), egui::Sense::hover());
                ui.painter().circle_filled(dot_rect.center(), 5.0, dot_color);
                ui.label(egui::RichText::new(label).size(10.5).color(egui::Color32::from_gray(200)));
            });
        });
}

/// Watches P2PConnectionState for drops during an active game and renders a
/// "Waiting N s for reconnect" banner. Auto-fires FlagTimeoutEvent at 0.
pub fn opponent_disconnect_ui(
    mut contexts: bevy_egui::EguiContexts,
    p2p_conn: Option<Res<crate::multiplayer::network::p2p::P2PConnectionState>>,
    game_over: Res<crate::game::resources::GameOverState>,
    game_mode: Res<GameMode>,
    mut disc: ResMut<OpponentDisconnectState>,
    mut timeout_writer: bevy::prelude::MessageWriter<crate::game::events::FlagTimeoutEvent>,
    current_turn: Res<crate::game::resources::CurrentTurn>,
) {
    use crate::multiplayer::network::p2p::P2PConnectionStatus;
    if game_over.is_game_over() { return; }

    let is_online = matches!(
        *game_mode,
        GameMode::BraidMultiplayer | GameMode::MultiplayerCompetitive
    );
    if !is_online { return; }

    // Detect disconnect / reconnect
    if let Some(conn) = p2p_conn.as_ref() {
        match &conn.status {
            P2PConnectionStatus::InGame => {
                // Reconnected — clear state
                if disc.disconnected_at.is_some() && !disc.timed_out {
                    disc.disconnected_at = None;
                }
            }
            P2PConnectionStatus::Disconnected | P2PConnectionStatus::Error(_) => {
                if disc.disconnected_at.is_none() && !disc.timed_out {
                    disc.disconnected_at = Some(std::time::Instant::now());
                }
            }
            _ => {}
        }
    }

    let Some(_since) = disc.disconnected_at else { return };
    let remaining = disc.remaining_secs();

    // Auto-fire timeout
    if remaining == 0 && !disc.timed_out {
        disc.timed_out = true;
        disc.disconnected_at = None;
        let flagged = match current_turn.color {
            crate::rendering::pieces::PieceColor::White => "white",
            crate::rendering::pieces::PieceColor::Black => "black",
        };
        timeout_writer.write(crate::game::events::FlagTimeoutEvent {
            flagged_player: flagged.to_string(),
            remote: false,
        });
        return;
    }

    // Render banner
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let col_bg  = egui::Color32::from_rgba_unmultiplied(180, 60, 20, 220);
    let col_txt = egui::Color32::WHITE;

    egui::Window::new("opponent_disconnect_banner")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_TOP, [0.0, 50.0])
        .auto_sized()
        .frame(egui::Frame::default().fill(col_bg).corner_radius(8.0).inner_margin(12.0))
        .show(ctx, |ui| {
            ui.label(egui::RichText::new(
                format!("Opponent disconnected — waiting {}s for reconnect…", remaining)
            ).size(13.0).color(col_txt).strong());
        });
}

// ── Check sound cue ───────────────────────────────────────────────────────────

/// Plays the check sound when the game phase transitions to Check.
pub fn play_check_sound_system(
    mut commands: Commands,
    game_phase: Res<crate::game::resources::CurrentGamePhase>,
    sounds: Option<Res<crate::game::resources::GameSounds>>,
    settings: Res<crate::core::GameSettings>,
) {
    use crate::game::components::GamePhase;
    if !game_phase.is_changed() { return; }
    if settings.muted { return; }
    let Some(s) = sounds else { return };
    if game_phase.0 == GamePhase::Check {
        commands.spawn(bevy::audio::AudioPlayer::new(s.check.clone()));
    }
}

// ── Blindfold mode toggle ─────────────────────────────────────────────────────

/// Toggle blindfold mode via Ctrl+B.
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

// ── Active tournament sidebar widget ─────────────────────────────────────────

/// Small floating panel shown when the player is in an active tournament.
/// Displays last match result and waiting status.
#[cfg(feature = "solana")]
pub fn tournament_sidebar_widget(
    mut contexts: bevy_egui::EguiContexts,
    tournament: Option<Res<crate::multiplayer::solana::tournament::TournamentClientState>>,
    lobby: Option<Res<crate::states::tournament_menu::TournamentLobbyState>>,
) {
    let Some(tc) = tournament else { return };
    if tc.active_tournament_id.is_none() { return };

    let Ok(ctx) = contexts.ctx_mut() else { return };

    let bg = egui::Color32::from_rgba_unmultiplied(12, 20, 30, 210);
    let border = egui::Color32::from_rgba_unmultiplied(180, 140, 0, 180);

    egui::Window::new("tournament_widget")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::RIGHT_TOP, [-12.0, 50.0])
        .auto_sized()
        .frame(
            egui::Frame::default()
                .fill(bg)
                .stroke(egui::Stroke::new(1.0, border))
                .corner_radius(8.0)
                .inner_margin(10.0),
        )
        .show(ctx, |ui| {
            ui.set_max_width(180.0);
            ui.label(egui::RichText::new("TOURNAMENT").size(10.0).strong()
                .color(egui::Color32::GOLD));
            ui.add_space(4.0);

            if tc.waiting_for_next_match {
                ui.label(egui::RichText::new("Waiting for next round…")
                    .size(11.0).italics().color(egui::Color32::from_rgb(100, 200, 255)));
            }
            if let Some(ref result) = tc.last_match_result {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Last result:").size(10.0)
                        .color(egui::Color32::from_gray(160)));
                    let col = match result.as_str() {
                        "1-0" => egui::Color32::from_rgb(100, 220, 100),
                        "0-1" => egui::Color32::from_rgb(220, 100, 100),
                        _     => egui::Color32::from_rgb(220, 200, 100),
                    };
                    ui.label(egui::RichText::new(result).size(11.0).strong().color(col));
                });
            }
            if let Some(slot) = tc.my_slot {
                ui.label(egui::RichText::new(format!("Slot {}", slot + 1)).size(10.0)
                    .color(egui::Color32::from_gray(140)));
            }
            if !tc.status_message.is_empty() {
                ui.add_space(2.0);
                ui.label(egui::RichText::new(&tc.status_message).size(10.0)
                    .color(egui::Color32::from_gray(180)).italics());
            }

            // Swiss standings (collapsible)
            if let Some(lobby_state) = lobby.as_ref() {
                if let Some(standings) = &lobby_state.swiss_standings {
                    if !standings.is_empty() {
                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(4.0);
                        let round_label = match (lobby_state.swiss_current_round, lobby_state.swiss_total_rounds) {
                            (Some(cur), Some(tot)) => format!("Round {}/{}", cur, tot),
                            (Some(cur), None) => format!("Round {}", cur),
                            _ => "Standings".to_string(),
                        };
                        ui.label(egui::RichText::new(round_label).size(10.0)
                            .strong().color(egui::Color32::from_gray(200)));
                        ui.add_space(2.0);
                        egui::Grid::new("swiss_standings_grid")
                            .num_columns(3)
                            .spacing([4.0, 2.0])
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new("Player").size(9.0).color(egui::Color32::from_gray(140)));
                                ui.label(egui::RichText::new("Pts").size(9.0).color(egui::Color32::from_gray(140)));
                                ui.label(egui::RichText::new("BH").size(9.0).color(egui::Color32::from_gray(140)));
                                ui.end_row();
                                for (rank, s) in standings.iter().enumerate().take(8) {
                                    let name_short = if s.player.len() > 10 {
                                        format!("{}.", &s.player[..9])
                                    } else {
                                        s.player.clone()
                                    };
                                    let row_col = if rank == 0 {
                                        egui::Color32::GOLD
                                    } else {
                                        egui::Color32::from_gray(200)
                                    };
                                    ui.label(egui::RichText::new(format!("{}. {}", rank + 1, name_short)).size(9.0).color(row_col));
                                    ui.label(egui::RichText::new(format!("{:.1}", s.score as f32 / 2.0)).size(9.0).color(row_col));
                                    ui.label(egui::RichText::new(format!("{:.1}", s.buchholz as f32 / 10.0)).size(9.0).color(egui::Color32::from_gray(150)));
                                    ui.end_row();
                                }
                            });

                        // Round progress dots
                        if let (Some(cur), Some(tot)) = (lobby_state.swiss_current_round, lobby_state.swiss_total_rounds) {
                            let tot = tot as usize;
                            let cur = cur as usize;
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                for r in 1..=tot {
                                    let done = r < cur;
                                    let active = r == cur;
                                    let size = egui::Vec2::splat(8.0);
                                    let (dot_rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                                    let center = dot_rect.center();
                                    let p = ui.painter();
                                    if done {
                                        p.circle_filled(center, 4.0, egui::Color32::GOLD);
                                    } else if active {
                                        p.circle_filled(center, 4.0, egui::Color32::from_rgb(100, 200, 255));
                                    } else {
                                        p.circle_stroke(center, 3.5, egui::Stroke::new(1.0, egui::Color32::from_gray(100)));
                                    }
                                    ui.add_space(2.0);
                                }
                            });
                        }
                    }
                }
            }
        });
}

// ── Increment flash tick ──────────────────────────────────────────────────────

/// Triggers and ticks the `IncrementFlash` resource so the "+Xs" label pulses
/// after each move when the time control has an increment.
pub fn increment_flash_system(
    mut flash: ResMut<IncrementFlash>,
    mut move_events: bevy::prelude::MessageReader<crate::game::events::MoveMadeEvent>,
    active_tc: Res<crate::game::resources::active_time_control::ActiveTimeControl>,
    time: Res<Time>,
) {
    flash.tick(time.delta_secs());

    if active_tc.control.increment_seconds() == 0 { return; }

    for ev in move_events.read() {
        let white_gained = ev.player == "white";
        flash.trigger(white_gained);
    }
}

// ── Resign / Offer Draw buttons ───────────────────────────────────────────────

/// Floating bottom-left panel with Resign and Offer Draw buttons.
/// Resign fires `ResignEvent` (winner = opponent).
/// Offer Draw fires `DrawOfferEvent` (only in online modes).

// -- Item 3: Session key expiry banner ----------------------------------------

/// Renders a dismissible warning banner at the top of the game screen when the
/// session key is within 24 h of expiry.
#[cfg(feature = "solana")]
pub fn session_expiry_banner(
    mut contexts: bevy_egui::EguiContexts,
    warning: Option<Res<crate::multiplayer::solana::integration::systems::SessionExpiryWarning>>,
    mut commands: Commands,
) {
    let Some(warn) = warning else { return };
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let bg = egui::Color32::from_rgba_unmultiplied(180, 100, 0, 220);
    let text_col = egui::Color32::from_rgb(255, 240, 180);

    egui::Window::new("session_expiry_banner")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_TOP, [0.0, 8.0])
        .fixed_size([520.0, 40.0])
        .frame(
            egui::Frame::default()
                .fill(bg)
                .corner_radius(6.0)
                .inner_margin(8.0),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "⚠  Session key expires in {} h — re-authorize in wallet settings to continue playing without popups.",
                        warn.expires_in_hours
                    ))
                    .size(12.0)
                    .color(text_col),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("✕").clicked() {
                        commands.remove_resource::<crate::multiplayer::solana::integration::systems::SessionExpiryWarning>();
                    }
                });
            });
        });
}

// ── Disconnect recovery banner ────────────────────────────────────────────────

/// Shown when an online game loses its P2P connection.  Offers a one-click
/// attempt to re-fetch game state from the backend so the game can resume.
pub fn disconnect_recovery_banner(
    mut contexts: bevy_egui::EguiContexts,
    game_mode: Res<crate::core::GameMode>,
    p2p_conn: Option<Res<crate::multiplayer::network::p2p::P2PConnectionState>>,
    game_over: Res<crate::game::resources::GameOverState>,
) {
    let is_online = matches!(
        *game_mode,
        crate::core::GameMode::BraidMultiplayer | crate::core::GameMode::MultiplayerCompetitive
    );
    if !is_online { return; }
    if game_over.is_game_over() { return; }

    let disconnected = p2p_conn.as_ref().map(|c| matches!(
        c.status,
        crate::multiplayer::network::p2p::P2PConnectionStatus::Disconnected
            | crate::multiplayer::network::p2p::P2PConnectionStatus::Error(_)
    )).unwrap_or(false);

    if !disconnected { return; }

    let Ok(ctx) = contexts.ctx_mut() else { return };

    egui::Window::new("disconnect_banner")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_TOP, [0.0, 56.0])
        .fixed_size([400.0, 38.0])
        .frame(
            egui::Frame::default()
                .fill(egui::Color32::from_rgba_unmultiplied(100, 20, 20, 230))
                .corner_radius(6.0)
                .inner_margin(8.0),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(
                    egui::RichText::new("Connection lost — attempting to reconnect…")
                        .size(12.0)
                        .color(egui::Color32::from_rgb(255, 180, 180)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Refresh state").clicked() {
                        if let Some(conn) = p2p_conn.as_ref() {
                            if let Some(game_id) = conn.game_id {
                                let base = crate::multiplayer::network::vps::vps_base();
                                std::thread::spawn(move || {
                                    let url = format!("{}/api/games/{}/state", base, game_id);
                                    if let Ok(resp) = reqwest::blocking::get(&url) {
                                        info!("[RECONNECT] Fetched state for game {}: {}", game_id, resp.status());
                                    }
                                });
                            }
                        }
                    }
                });
            });
        });
}

/// Initiates avatar fetches for both players and drains channel bytes into PendingBytes entries.
/// Actual egui texture creation happens inside game_status_ui (needs egui context).
pub fn avatar_fetch_system(
    players: Res<crate::game::resources::Players>,
    mut avatar_cache: ResMut<AvatarCache>,
) {
    avatar_cache.fetch_if_absent(&players.player_1.name);
    avatar_cache.fetch_if_absent(&players.player_2.name);
    avatar_cache.drain_channel();
}

