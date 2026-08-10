//! In-game UI for chess game display

use crate::core::GameMode;
use crate::game::components::GamePhase;
use crate::game::resources::system_params::GameStateParams;
use crate::rendering::pieces::PieceColor;
use crate::ui::styles::*;
use crate::ui::system_params::GameUIParams;
use bevy::prelude::*;
use bevy_egui::egui;
use std::collections::{HashMap, VecDeque};
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

/// Cap on distinct avatars held in memory at once — without this, a long
/// session that browses lobbies/spectates/views standings accumulates one
/// GPU texture per unique player name forever. Eviction is FIFO by first
/// insertion (not true LRU): simple, and re-fetching an evicted-but-still-
/// relevant avatar just costs one HTTP call, not a correctness issue.
const MAX_AVATAR_CACHE_ENTRIES: usize = 200;

/// Caches player avatars fetched from the backend.
/// Key: player name / wallet address.
#[derive(Resource)]
pub struct AvatarCache {
    pub entries: HashMap<String, AvatarEntry>,
    insertion_order: VecDeque<String>,
    rx: std::sync::Mutex<Receiver<(String, Vec<u8>)>>,
    pub tx: Sender<(String, Vec<u8>)>,
}

impl Default for AvatarCache {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            entries: HashMap::new(),
            insertion_order: VecDeque::new(),
            rx: std::sync::Mutex::new(rx),
            tx,
        }
    }
}

impl AvatarCache {
    /// Kick off a background fetch for `name` if not already in progress.
    pub fn fetch_if_absent(&mut self, name: &str) {
        if self.entries.contains_key(name) {
            return;
        }
        if self.entries.len() >= MAX_AVATAR_CACHE_ENTRIES {
            if let Some(oldest) = self.insertion_order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
        self.entries.insert(name.to_string(), AvatarEntry::Loading);
        self.insertion_order.push_back(name.to_string());
        let tx = self.tx.clone();
        let key = name.to_string();
        let base = crate::multiplayer::network::vps::vps_base();
        std::thread::spawn(move || {
            let url = format!("{}/api/players/{}/avatar", base, key);
            let result = reqwest::blocking::get(&url).and_then(|r| r.bytes().map(|b| b.to_vec()));
            match result {
                Ok(bytes) if !bytes.is_empty() => {
                    let _ = tx.send((key, bytes));
                }
                _ => {
                    let _ = tx.send((key, Vec::new()));
                }
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
                        let tex = ctx.load_texture(
                            "player_avatar",
                            color_img,
                            egui::TextureOptions::LINEAR,
                        );
                        *entry = AvatarEntry::Loaded(tex);
                    }
                    Err(_) => {
                        *entry = AvatarEntry::Failed;
                    }
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
        let Ok(ctx_ref) = params.contexts.ctx_mut() else {
            return;
        };
        ctx_ref.clone()
    };

    // Decode any pending avatar bytes into egui textures.
    params.avatar_cache.flush_pending(&ctx);

    // === TOP BAR ===
    // Declared first so it reserves the full-width strip before the
    // left/right SidePanels and the board area are laid out beneath it —
    // egui panels shrink whatever the "remaining rect" is at the time
    // they're declared, so ordering here matters.
    crate::ui::game::top_bar::render_game_top_bar(&ctx, &mut params);

    // === CENTER PLAYER BARS ===
    // Resolved once here (this system has full `GameUIParams` access) and
    // cached so the independent 2D board system can read it without
    // duplicating the identity/clock/capture resolution — see
    // `player_bar::PlayerBarsCache`. In 3D there's no board-owned
    // `CentralPanel` to embed the bars in, so they're rendered directly here
    // as floating overlays; in 2D, `render_2d_board` draws them itself
    // (later in the same frame) using the cached data.
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

    // === CHECK / CHECKMATE INDICATOR ===
    match params.game_state.game_phase.0 {
        GamePhase::Checkmate => render_checkmate_banner(&ctx, &params.game_state),
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

    // === LEFT PANEL ===
    // Game type / rated badge / time control, both players, inline chat
    // (online games only). Declared before the central board panel so the
    // board correctly reserves space for it — see left_panel.rs.
    egui::SidePanel::left("game_info_panel")
        .resizable(false)
        .show_separator_line(false)
        .exact_width(Layout::SIDE_PANEL_WIDTH)
        .frame(
            egui::Frame::default()
                .fill(UiColors::BG_OVERLAY)
                .inner_margin(egui::Margin::symmetric(8, 10))
                .stroke(egui::Stroke::NONE),
        )
        .show(&ctx, |ui| {
            crate::ui::game::left_panel::render_game_left_panel(ui, &mut params);
        });

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
                .show_separator_line(false)
                .show(&ctx, |ui| {
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
                                );
                            }
                        });
                });
        }
    }
}

// ── Lichess-style right panel helpers ────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::resolve_online_player_name;
    use crate::states::main_menu::PlayerIdentity;

    #[test]
    fn prefers_signed_in_username_for_local_hud_name() {
        let identity = PlayerIdentity {
            username: Some("Alice".to_string()),
            ..Default::default()
        };

        assert_eq!(resolve_online_player_name(Some(&identity), Some("Bob")), "Alice");
    }

    #[test]
    fn falls_back_to_profile_name_when_identity_has_no_username() {
        let identity = PlayerIdentity::default();

        assert_eq!(resolve_online_player_name(Some(&identity), Some("Bob")), "Bob");
    }
}

/// Resolve display name + ELO string for both colors from whichever source
/// applies to the current game (spectator feed, Solana competitive match, or
/// plain local `Players`). Shared by the right sidebar and the left game-info
/// panel so both show identical player identity data.
fn resolve_online_player_name(
    player_identity: Option<&crate::states::main_menu::PlayerIdentity>,
    fallback_name: Option<&str>,
) -> String {
    player_identity
        .and_then(|id| id.username.as_ref())
        .filter(|name| !name.trim().is_empty())
        .cloned()
        .or_else(|| fallback_name.map(str::to_string))
        .unwrap_or_default()
}

pub(crate) fn resolve_player_names(
    params: &crate::ui::system_params::game_ui::GameUIParams,
    local_color: PieceColor,
    is_spectating: bool,
) -> (String, String, String, String) {
    let white_name: String;
    let white_elo: String;
    let black_name: String;
    let black_elo: String;

    if is_spectating {
        let w = params.spectator_mode.white_player.as_ref();
        let b = params.spectator_mode.black_player.as_ref();
        white_name = w
            .map(|p| p.username.clone())
            .unwrap_or_else(|| "White".to_string());
        white_elo = w.map(|p| format!("{}", p.rating)).unwrap_or_default();
        black_name = b
            .map(|p| p.username.clone())
            .unwrap_or_else(|| "Black".to_string());
        black_elo = b.map(|p| format!("{}", p.rating)).unwrap_or_default();
    } else if *params.game_mode == crate::core::GameMode::SinglePlayer {
        // vs Computer: one side is the AI (named "Computer", ELO from the
        // selected difficulty), the other is the local human — identified by
        // their logged-in name/ELO (on-chain and/or Lichess) when signed in,
        // or just their color when playing as a guest.
        let ai_color = params.ai_params.ai_config.mode.ai_color();
        let human_color = match ai_color {
            PieceColor::White => PieceColor::Black,
            PieceColor::Black => PieceColor::White,
        };
        let ai_name = "Computer".to_string();
        let ai_elo = params
            .ai_params
            .ai_config
            .difficulty
            .elo_label()
            .to_string();
        let human_name = params
            .player_identity
            .as_ref()
            .filter(|id| id.username.is_some())
            .map(|id| id.display_name().to_string())
            .unwrap_or_else(|| match human_color {
                PieceColor::White => "White".to_string(),
                PieceColor::Black => "Black".to_string(),
            });
        let human_elo = params
            .player_identity
            .as_ref()
            .map(|id| id.hud_elo_label())
            .unwrap_or_default();
        match ai_color {
            PieceColor::White => {
                white_name = ai_name;
                white_elo = ai_elo;
                black_name = human_name;
                black_elo = human_elo;
            }
            PieceColor::Black => {
                white_name = human_name;
                white_elo = human_elo;
                black_name = ai_name;
                black_elo = ai_elo;
            }
        }
    } else {
        #[cfg(feature = "solana")]
        {
            if let (Some(ref profile), Some(ref comp)) = (
                params.solana_profile.as_ref(),
                params.competitive_match.as_ref(),
            ) {
                // The local player's name should come from the sign-in
                // identity (`PlayerIdentity`, populated as soon as the wallet
                // UI hands off) rather than `SolanaProfile.username`, which
                // stays empty until a separate async VPS fetch resolves —
                // that race is why the local panel showed "Player".
                let local_name = resolve_online_player_name(
                    params.player_identity.as_ref().map(|id| id.as_ref()),
                    Some(profile.username.as_str()),
                );
                let is_white_local = local_color == PieceColor::White;
                if is_white_local {
                    white_name = local_name;
                    white_elo = format!("{}", profile.elo);
                    black_name = comp.opponent_username.clone();
                    black_elo = format!("{}", comp.opponent_elo);
                } else {
                    black_name = local_name;
                    black_elo = format!("{}", profile.elo);
                    white_name = comp.opponent_username.clone();
                    white_elo = format!("{}", comp.opponent_elo);
                }
            } else {
                white_name = params.players.player_1.name.clone();
                white_elo = String::new();
                black_name = params.players.player_2.name.clone();
                black_elo = String::new();
            }
        }
        #[cfg(not(feature = "solana"))]
        {
            white_name = params.players.player_1.name.clone();
            white_elo = String::new();
            black_name = params.players.player_2.name.clone();
            black_elo = String::new();
        }
    }

    (white_name, white_elo, black_name, black_elo)
}

/// "MOVES" header, scrollable move list, then Resign / Offer Draw controls.
/// Rendered in the top-left panel, below the match-info card. Player
/// identity/clock/captures live in the center player bars above/below the
/// board — see `crate::ui::game::player_bar`.
pub(crate) fn render_moves_and_controls(
    ui: &mut egui::Ui,
    params: &mut crate::ui::system_params::game_ui::GameUIParams,
) {
    use crate::game::resources::TurnPhase;

    {
        let is_online = matches!(
            *params.game_mode,
            crate::core::GameMode::OnlineMultiplayer
                | crate::core::GameMode::MultiplayerCompetitive
        );

        // ── MOVES ─────────────────────────────────────────────────────────────
        StyledPanel::sidebar_row()
            .inner_margin(egui::Margin::symmetric(12, 6))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("MOVES")
                        .size(12.0)
                        .strong()
                        .color(UiColors::TEXT_TERTIARY),
                );
            });

        // Fixed height — the list scrolls internally and never grows the
        // panel as more moves are played.
        const MOVE_LIST_HEIGHT: f32 = 260.0;

        StyledPanel::sidebar_card()
            .fill(UiColors::BG_MID)
            .inner_margin(egui::Margin::symmetric(0, 0))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("game_moves_scroll")
                    .max_height(MOVE_LIST_HEIGHT)
                    .min_scrolled_height(MOVE_LIST_HEIGHT)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        StyledPanel::sidebar_row()
                            .inner_margin(egui::Margin::symmetric(12, 8))
                            .show(ui, |ui| {
                                render_move_list_paired(ui, &params.move_history);
                            });
                    });
            });

        ui.add_space(10.0);

        // ── CONTROLS ─────────────────────────────────────────────────────────
        StyledPanel::sidebar_row()
            .inner_margin(egui::Margin::symmetric(12, 0))
            .show(ui, |ui| {
                let is_game_over = params.game_state.game_over.is_game_over();
                let is_waiting = params.turn_ctx.phase == TurnPhase::WaitingForInput;

                if !is_game_over {
                    if is_online {
                        let draw_offered = params.pending_draw.from_player.is_some();
                        if StyledButton::outline_pill(
                            ui,
                            if draw_offered {
                                "DRAW SENT"
                            } else {
                                "OFFER DRAW"
                            },
                            egui::Color32::from_rgb(210, 180, 90),
                            true,
                            !draw_offered,
                        )
                        .clicked()
                        {
                            let player = match params.current_turn.color {
                                PieceColor::White => "white".to_string(),
                                PieceColor::Black => "black".to_string(),
                            };
                            params
                                .draw_writer
                                .write(crate::game::events::DrawOfferEvent {
                                    player,
                                    remote: false,
                                });

                            // Competitive on-chain games also need the offer
                            // recorded on the `Game` account itself — the P2P
                            // event above only gives the opponent's client
                            // instant UI feedback, it never touches chain.
                            #[cfg(feature = "solana")]
                            if let (Some(wallet_pubkey), Some(game_id), Some(rpc_url)) = (
                                params.solana_wallet.as_ref().and_then(|w| w.pubkey),
                                params.solana_sync.as_ref().and_then(|s| s.game_id),
                                params.solana_sync.as_ref().map(|s| s.rpc_url.clone()),
                            ) {
                                crate::multiplayer::solana::lobby::spawn_offer_draw(
                                    rpc_url,
                                    wallet_pubkey,
                                    game_id,
                                );
                            }
                        }
                        ui.add_space(8.0);
                    }

                    if StyledButton::outline_pill(ui, "RESIGN", UiColors::DANGER, true, is_waiting)
                        .clicked()
                    {
                        let winner = match params.current_turn.color {
                            PieceColor::White => "black".to_string(),
                            PieceColor::Black => "white".to_string(),
                        };
                        params
                            .resign_writer
                            .write(crate::game::events::ResignEvent {
                                winner,
                                remote: false,
                            });
                    }
                }
            });
    }
}

pub(crate) fn render_compact_user_row(
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
        let initial = display_name
            .chars()
            .next()
            .unwrap_or('?')
            .to_uppercase()
            .next()
            .unwrap_or('?');
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
        ui.label(
            egui::RichText::new(display_name)
                .size(13.0)
                .color(UiColors::TEXT_PRIMARY)
                .strong(),
        );
        // ELO — right-aligned if present (never show a bare "0" placeholder)
        if !elo.is_empty() && elo != "0" {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(elo)
                        .size(11.0)
                        .color(UiColors::TEXT_TERTIARY),
                );
            });
        }
    });
}

pub(crate) fn render_clock_bar(
    ui: &mut egui::Ui,
    time_secs: f32,
    is_active: bool,
    flagged: bool,
    pulse_alpha: u8,
) {
    let low_time = time_secs < 10.0;
    let bg_fill = if is_active && low_time {
        egui::Color32::from_rgba_unmultiplied(130, 18, 18, 230)
    } else {
        egui::Color32::from_rgba_unmultiplied(22, 22, 28, 200)
    };
    let (font_size, color) = if is_active && low_time {
        (24.0_f32, egui::Color32::from_rgb(255, 90, 90))
    } else {
        (
            24.0_f32,
            egui::Color32::from_gray(is_active as u8 * 140 + 100),
        )
    };

    egui::Frame::default()
        .fill(bg_fill)
        .corner_radius(egui::CornerRadius::same(6))
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
        .min_col_width(44.0)
        .spacing([10.0, 4.0])
        .show(ui, |ui| {
            for move_num in 1..=((total + 1) / 2) {
                let white_idx = (move_num - 1) * 2;
                let black_idx = white_idx + 1;
                ui.label(
                    egui::RichText::new(format!("{}.", move_num))
                        .size(13.0)
                        .color(UiColors::TEXT_TERTIARY),
                );
                if white_idx < total {
                    let mv = &moves[white_idx];
                    ui.label(
                        egui::RichText::new(move_notation(history, white_idx, mv))
                            .size(16.0)
                            .color(UiColors::TEXT_PRIMARY)
                            .strong(),
                    );
                } else {
                    ui.label("");
                }
                if black_idx < total {
                    let mv = &moves[black_idx];
                    ui.label(
                        egui::RichText::new(move_notation(history, black_idx, mv))
                            .size(16.0)
                            .color(UiColors::TEXT_SECONDARY)
                            .strong(),
                    );
                } else if white_idx < total {
                    ui.label(
                        egui::RichText::new("…")
                            .size(16.0)
                            .color(UiColors::TEXT_TERTIARY),
                    );
                } else {
                    ui.label("");
                }
                ui.end_row();
            }
        });
}

/// Notation for the move at `index` — prefers the properly-disambiguated SAN
/// recorded via `MoveHistory::add_move_with_san`, falling back to the
/// simplified hand-rolled notation only for entries that predate/skip it
/// (e.g. moves constructed directly in tests).
fn move_notation(
    history: &crate::game::resources::history::MoveHistory,
    index: usize,
    mv: &crate::game::components::MoveRecord,
) -> String {
    history
        .san_at(index)
        .map(str::to_string)
        .unwrap_or_else(|| format_move_algebraic(mv))
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
        egui::Color32::from_rgb(70, 130, 200),  // blue
        egui::Color32::from_rgb(60, 160, 100),  // green
        egui::Color32::from_rgb(200, 90, 60),   // red-orange
        egui::Color32::from_rgb(140, 80, 200),  // purple
        egui::Color32::from_rgb(200, 150, 40),  // amber
        egui::Color32::from_rgb(40, 170, 170),  // teal
        egui::Color32::from_rgb(200, 70, 140),  // pink
        egui::Color32::from_rgb(100, 100, 120), // slate
    ];
    colors[(hash as usize) % colors.len()]
}

/// Render captured piece symbols as a compact tray row.
/// `pieces` = pieces captured BY this side (the opponent's piece type).
/// `is_dark` true = render as dark pieces (captured by white), false = light pieces (captured by black).
pub(crate) fn render_captured_pieces_tray(
    ui: &mut egui::Ui,
    pieces: &[crate::rendering::pieces::PieceType],
    is_dark: bool,
) {
    use crate::rendering::pieces::PieceType;
    if pieces.is_empty() {
        return;
    }

    let (sym_color, _bg) = if is_dark {
        (
            egui::Color32::from_gray(30),
            egui::Color32::from_rgba_unmultiplied(230, 220, 195, 60),
        )
    } else {
        (
            egui::Color32::from_gray(225),
            egui::Color32::from_rgba_unmultiplied(50, 50, 60, 60),
        )
    };

    let mut sorted = pieces.to_vec();
    let order = |p: &PieceType| match p {
        PieceType::Queen => 0,
        PieceType::Rook => 1,
        PieceType::Bishop => 2,
        PieceType::Knight => 3,
        PieceType::Pawn => 4,
        PieceType::King => 5,
    };
    sorted.sort_by_key(order);

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for pt in &sorted {
            let sym = if is_dark {
                match pt {
                    PieceType::Queen => "♛",
                    PieceType::Rook => "♜",
                    PieceType::Bishop => "♝",
                    PieceType::Knight => "♞",
                    PieceType::Pawn => "♟",
                    PieceType::King => "♚",
                }
            } else {
                match pt {
                    PieceType::Queen => "♕",
                    PieceType::Rook => "♖",
                    PieceType::Bishop => "♗",
                    PieceType::Knight => "♘",
                    PieceType::Pawn => "♙",
                    PieceType::King => "♔",
                }
            };
            ui.label(egui::RichText::new(sym).size(17.0).color(sym_color));
        }
    });
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

/// Plain "Check" text at top-centre — no box, in the game's serif font.
fn render_check_banner(ctx: &egui::Context) {
    egui::Area::new("check_indicator".into())
        .order(egui::Order::Foreground)
        .interactable(false)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 24.0))
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new("Check")
                    .size(30.0)
                    .family(egui::FontFamily::Name("CinzelBold".into()))
                    .color(egui::Color32::from_rgb(224, 96, 64)),
            );
        });
}

/// Render a minimal checkmate pill — small, unobtrusive, top-center.
fn render_checkmate_banner(ctx: &egui::Context, game_state: &GameStateParams) {
    let (winner_text, _) = match game_state.game_over.winner() {
        Some(PieceColor::White) => ("White wins", egui::Color32::from_rgb(240, 240, 240)),
        Some(PieceColor::Black) => ("Black wins", egui::Color32::from_rgb(200, 200, 200)),
        None => ("Draw", egui::Color32::from_rgb(200, 200, 200)),
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
                .stroke(egui::Stroke::new(
                    1.0,
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30),
                )),
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
    let Some(from) = pending.from_player.as_ref() else {
        return;
    };
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
                    // horizontal_centered only centers vertically — pad to center the pair.
                    ui.add_space(((ui.available_width() - (120.0 * 2.0 + 12.0)) / 2.0).max(0.0));

                    let local_player = p2p_conn
                        .as_ref()
                        .and_then(|c| c.player_color)
                        .map(|col| match col {
                            crate::rendering::pieces::PieceColor::White => "white",
                            crate::rendering::pieces::PieceColor::Black => "black",
                        })
                        .unwrap_or("white")
                        .to_string();

                    if ui
                        .add_sized(
                            [120.0, 38.0],
                            egui::Button::new(
                                egui::RichText::new("Accept")
                                    .size(13.0)
                                    .color(egui::Color32::WHITE)
                                    .strong(),
                            )
                            .fill(egui::Color32::from_rgb(34, 100, 50))
                            .stroke(egui::Stroke::NONE)
                            .corner_radius(8.0),
                        )
                        .clicked()
                    {
                        draw_response.write(crate::game::events::DrawResponseEvent {
                            player: local_player.clone(),
                            accepted: true,
                            remote: false,
                        });
                    }

                    if ui
                        .add_sized(
                            [120.0, 38.0],
                            egui::Button::new(
                                egui::RichText::new("Decline")
                                    .size(13.0)
                                    .color(egui::Color32::WHITE),
                            )
                            .fill(UiColors::BTN_POPUP_DARK)
                            .stroke(egui::Stroke::NONE)
                            .corner_radius(8.0),
                        )
                        .clicked()
                    {
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

/// Overlay system: shows an Accept/Decline banner when the opponent has offered a rematch.
pub fn rematch_offer_ui(
    mut contexts: bevy_egui::EguiContexts,
    mut pending: ResMut<crate::game::systems::network_move::PendingRematchOffer>,
    mut rematch_response: bevy::prelude::MessageWriter<crate::game::events::RematchResponseEvent>,
    p2p_conn: Option<Res<crate::multiplayer::network::p2p::P2PConnectionState>>,
) {
    let Some(from) = pending.from_player.clone() else {
        return;
    };
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
                    // horizontal_centered only centers vertically — pad to center the pair.
                    ui.add_space(((ui.available_width() - (120.0 * 2.0 + 12.0)) / 2.0).max(0.0));

                    if ui
                        .add_sized(
                            [120.0, 38.0],
                            egui::Button::new(
                                egui::RichText::new("Accept")
                                    .size(13.0)
                                    .color(egui::Color32::WHITE)
                                    .strong(),
                            )
                            .fill(egui::Color32::from_rgb(34, 100, 50))
                            .stroke(egui::Stroke::NONE)
                            .corner_radius(8.0),
                        )
                        .clicked()
                    {
                        rematch_response.write(crate::game::events::RematchResponseEvent {
                            player: local_player.clone(),
                            accepted: true,
                            remote: false,
                        });
                        pending.from_player = None;
                    }

                    if ui
                        .add_sized(
                            [120.0, 38.0],
                            egui::Button::new(
                                egui::RichText::new("Decline")
                                    .size(13.0)
                                    .color(egui::Color32::WHITE),
                            )
                            .fill(UiColors::BTN_POPUP_DARK)
                            .stroke(egui::Stroke::NONE)
                            .corner_radius(8.0),
                        )
                        .clicked()
                    {
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
        self.disconnected_at
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0)
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
    if !matches!(
        *game_mode,
        GameMode::OnlineMultiplayer | GameMode::MultiplayerCompetitive
    ) {
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
                let (dot_rect, _) =
                    ui.allocate_exact_size(egui::Vec2::splat(10.0), egui::Sense::hover());
                ui.painter()
                    .circle_filled(dot_rect.center(), 5.0, dot_color);
                ui.label(
                    egui::RichText::new(label)
                        .size(10.5)
                        .color(egui::Color32::from_gray(200)),
                );
            });
        });
}

/// Watches P2PConnectionState (and, independently, the opponent's backend
/// `/presence` heartbeat — see `OpponentLivenessState`) for drops during an
/// active game and renders a "Waiting N s for reconnect" banner. Auto-fires
/// FlagTimeoutEvent at 0.
pub fn opponent_disconnect_ui(
    mut contexts: bevy_egui::EguiContexts,
    p2p_conn: Option<Res<crate::multiplayer::network::p2p::P2PConnectionState>>,
    liveness: Res<crate::multiplayer::social::OpponentLivenessState>,
    game_over: Res<crate::game::resources::GameOverState>,
    game_mode: Res<GameMode>,
    mut disc: ResMut<OpponentDisconnectState>,
    mut timeout_writer: bevy::prelude::MessageWriter<crate::game::events::FlagTimeoutEvent>,
    current_turn: Res<crate::game::resources::CurrentTurn>,
) {
    use crate::multiplayer::network::p2p::P2PConnectionStatus;
    if game_over.is_game_over() {
        return;
    }

    let is_online = matches!(
        *game_mode,
        GameMode::OnlineMultiplayer | GameMode::MultiplayerCompetitive
    );
    if !is_online {
        return;
    }

    // Detect disconnect / reconnect. Two independent signals, either one
    // enough to declare "gone": the P2P/gossip transport state (fast, but
    // only meaningful when a direct link was ever established) and the
    // opponent's backend presence heartbeat (slower, but works for
    // relay-only games too — see `check_opponent_presence`). Both must be
    // healthy again to clear the banner, so a lingering stale presence
    // sample can't mask a P2P reconnect declaring "all clear" too early.
    if let Some(conn) = p2p_conn.as_ref() {
        let is_gone = matches!(
            conn.status,
            P2PConnectionStatus::Disconnected | P2PConnectionStatus::Error(_)
        ) || liveness.is_opponent_stale();
        if is_gone {
            if disc.disconnected_at.is_none() && !disc.timed_out {
                disc.disconnected_at = Some(std::time::Instant::now());
            }
        } else if conn.status == P2PConnectionStatus::InGame {
            // Reconnected — clear state
            if disc.disconnected_at.is_some() && !disc.timed_out {
                disc.disconnected_at = None;
            }
        }
    }

    let Some(_since) = disc.disconnected_at else {
        return;
    };
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
    let col_bg = egui::Color32::from_rgba_unmultiplied(180, 60, 20, 220);
    let col_txt = egui::Color32::WHITE;

    egui::Window::new("opponent_disconnect_banner")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_TOP, [0.0, 50.0])
        .auto_sized()
        .frame(
            egui::Frame::default()
                .fill(col_bg)
                .corner_radius(8.0)
                .inner_margin(12.0),
        )
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new(format!(
                    "Opponent disconnected — waiting {}s for reconnect…",
                    remaining
                ))
                .size(13.0)
                .color(col_txt)
                .strong(),
            );
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
    if !game_phase.is_changed() {
        return;
    }
    if settings.muted {
        return;
    }
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
    ai_config: Option<Res<crate::game::ai::resource::ChessAIResource>>,
) {
    let Some(tc) = tournament else { return };
    if tc.active_tournament_id.is_none() {
        return;
    };
    // `active_tournament_id` only ever clears on an explicit "Leave"/"Cancel"
    // click (screens.rs) — nothing clears it when the tournament actually
    // finishes for this player, so it can stay `Some(..)` indefinitely after
    // a tournament the player registered for has long since ended. Rather
    // than trusting that stale flag alone, also require the current game to
    // not be an unrelated vs-AI match — this widget showing "TOURNAMENT" /
    // "waiting for next round" over a vs-Computer game is exactly that
    // staleness surfacing, not a legitimate reminder.
    if let Some(ai_config) = &ai_config {
        if matches!(
            ai_config.mode,
            crate::game::ai::resource::GameMode::VsAI { .. }
        ) {
            return;
        }
    }

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
            ui.label(
                egui::RichText::new("TOURNAMENT")
                    .size(10.0)
                    .strong()
                    .color(egui::Color32::GOLD),
            );
            ui.add_space(4.0);

            if tc.waiting_for_next_match {
                ui.label(
                    egui::RichText::new("Waiting for next round…")
                        .size(11.0)
                        .italics()
                        .color(egui::Color32::from_rgb(100, 200, 255)),
                );
            }
            if let Some(ref result) = tc.last_match_result {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Last result:")
                            .size(10.0)
                            .color(egui::Color32::from_gray(160)),
                    );
                    let col = match result.as_str() {
                        "1-0" => egui::Color32::from_rgb(100, 220, 100),
                        "0-1" => egui::Color32::from_rgb(220, 100, 100),
                        _ => egui::Color32::from_rgb(220, 200, 100),
                    };
                    ui.label(egui::RichText::new(result).size(11.0).strong().color(col));
                });
            }
            if let Some(slot) = tc.my_slot {
                ui.label(
                    egui::RichText::new(format!("Slot {}", slot + 1))
                        .size(10.0)
                        .color(egui::Color32::from_gray(140)),
                );
            }
            if !tc.status_message.is_empty() {
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(&tc.status_message)
                        .size(10.0)
                        .color(egui::Color32::from_gray(180))
                        .italics(),
                );
            }

            // Swiss standings (collapsible)
            if let Some(lobby_state) = lobby.as_ref() {
                if let Some(standings) = &lobby_state.swiss_standings {
                    if !standings.is_empty() {
                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(4.0);
                        let round_label = match (
                            lobby_state.swiss_current_round,
                            lobby_state.swiss_total_rounds,
                        ) {
                            (Some(cur), Some(tot)) => format!("Round {}/{}", cur, tot),
                            (Some(cur), None) => format!("Round {}", cur),
                            _ => "Standings".to_string(),
                        };
                        ui.label(
                            egui::RichText::new(round_label)
                                .size(10.0)
                                .strong()
                                .color(egui::Color32::from_gray(200)),
                        );
                        ui.add_space(2.0);
                        egui::Grid::new("swiss_standings_grid")
                            .num_columns(3)
                            .spacing([4.0, 2.0])
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new("Player")
                                        .size(9.0)
                                        .color(egui::Color32::from_gray(140)),
                                );
                                ui.label(
                                    egui::RichText::new("Pts")
                                        .size(9.0)
                                        .color(egui::Color32::from_gray(140)),
                                );
                                ui.label(
                                    egui::RichText::new("BH")
                                        .size(9.0)
                                        .color(egui::Color32::from_gray(140)),
                                );
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
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{}. {}",
                                            rank + 1,
                                            name_short
                                        ))
                                        .size(9.0)
                                        .color(row_col),
                                    );
                                    ui.label(
                                        egui::RichText::new(format!("{:.1}", s.score as f32 / 2.0))
                                            .size(9.0)
                                            .color(row_col),
                                    );
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{:.1}",
                                            s.buchholz as f32 / 10.0
                                        ))
                                        .size(9.0)
                                        .color(egui::Color32::from_gray(150)),
                                    );
                                    ui.end_row();
                                }
                            });

                        // Round progress dots
                        if let (Some(cur), Some(tot)) = (
                            lobby_state.swiss_current_round,
                            lobby_state.swiss_total_rounds,
                        ) {
                            let tot = tot as usize;
                            let cur = cur as usize;
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                for r in 1..=tot {
                                    let done = r < cur;
                                    let active = r == cur;
                                    let size = egui::Vec2::splat(8.0);
                                    let (dot_rect, _) =
                                        ui.allocate_exact_size(size, egui::Sense::hover());
                                    let center = dot_rect.center();
                                    let p = ui.painter();
                                    if done {
                                        p.circle_filled(center, 4.0, egui::Color32::GOLD);
                                    } else if active {
                                        p.circle_filled(
                                            center,
                                            4.0,
                                            egui::Color32::from_rgb(100, 200, 255),
                                        );
                                    } else {
                                        p.circle_stroke(
                                            center,
                                            3.5,
                                            egui::Stroke::new(1.0, egui::Color32::from_gray(100)),
                                        );
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

    if active_tc.control.increment_seconds() == 0 {
        return;
    }

    for ev in move_events.read() {
        let white_gained = ev.player == "white";
        flash.trigger(white_gained);
    }
}

// ── Resign / Offer Draw buttons ───────────────────────────────────────────────

/// Floating bottom-left panel with Resign and Offer Draw buttons.
/// Resign fires `ResignEvent` (winner = opponent).
/// Offer Draw fires `DrawOfferEvent` (only in online modes).

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
        crate::core::GameMode::OnlineMultiplayer | crate::core::GameMode::MultiplayerCompetitive
    );
    if !is_online {
        return;
    }
    if game_over.is_game_over() {
        return;
    }

    let disconnected = p2p_conn
        .as_ref()
        .map(|c| {
            matches!(
                c.status,
                crate::multiplayer::network::p2p::P2PConnectionStatus::Disconnected
                    | crate::multiplayer::network::p2p::P2PConnectionStatus::Error(_)
            )
        })
        .unwrap_or(false);

    if !disconnected {
        return;
    }

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
                                        info!(
                                            "[RECONNECT] Fetched state for game {}: {}",
                                            game_id,
                                            resp.status()
                                        );
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
