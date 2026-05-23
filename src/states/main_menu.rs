#![allow(dead_code)]
//! Main menu plugin with polished UI.
//!
//! Displays the primary game menu with options to:
//! - Start a new game (with mode selection)
//! - Access settings
//! - View statistics
//! - Exit the application
//!
//! Features styled UI components from the theme system and an optional
//! animated 3D background scene. Heavier UI is split across sibling
//! submodules under `main_menu/`:
//! - [`screens`] — Solana/Braid lobby, tournament browser, host-config and
//!   waiting screens, plus the lobby-selection / spectator / join popups
//!   driven by `MenuState` transitions.
//! - [`navbar`] — top navigation bar and link/button helpers.
//! - [`sections`] — PLAY / QUICK PLAY / NEWS / LEARN / TOURNAMENTS / UPDATES
//!   cards that make up the website-style body.
//! - [`modals`] — AI setup modal and controls popup reached from the navbar.
//!
//! This root file keeps the Bevy plugin, shared resources (`CompetitiveMenuState`,
//! `PlayerIdentity`, `P2PHostState`, cached textures), camera wiring, font
//! loading, and the top-level [`main_menu_ui`] / [`render_website_menu`]
//! orchestrators that call into the submodules above.

use crate::assets::{
    check_asset_loading, handle_asset_loading_errors, handle_untyped_asset_loading_errors,
    start_asset_loading,
};
use crate::core::GameState;
use crate::ui::system_params::MainMenuUIContext;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use std::sync::Arc;

#[path = "main_menu/screens.rs"]
mod screens;
#[path = "main_menu/modals.rs"]
mod modals;
#[path = "main_menu/new_menu.rs"]
mod new_menu;
#[path = "main_menu/board_animation.rs"]
mod board_animation;

use screens::*;
use modals::{render_ai_setup_modal, render_controls_popup};
use new_menu::{
    orbit_camera_system, render_new_style_panel, render_solana_splash,
    render_wallet_hud, setup_menu_fog, spawn_menu_bg_board, spawn_menu_bg_lights,
    spawn_menu_bg_pieces,
};
pub use new_menu::NewMenuPanel;

/// Visual style marker — only the 3D board style exists now.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MenuStyle {
    #[default]
    New,
}

/// Plugin for main menu state.
pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MenuStyle>()
            .init_resource::<new_menu::MenuCameraOrbit>()
            .init_resource::<new_menu::MenuBgPiecesSpawned>()
            .init_resource::<new_menu::NewMenuPanel>()
            .init_resource::<board_animation::BoardAnimator>()
            .init_resource::<WalletBridgePoller>()
            .init_resource::<FontsLoaded>()
            .add_systems(
                OnEnter(GameState::MainMenu),
                (
                    setup_menu_camera,
                    start_asset_loading,
                    spawn_menu_bg_board,
                    spawn_menu_bg_lights,
                    setup_menu_fog,
                ),
            )
            .add_systems(
                OnExit(GameState::MainMenu),
                |mut spawned: ResMut<new_menu::MenuBgPiecesSpawned>,
                 mut fl: ResMut<FontsLoaded>,
                 mut anim: ResMut<board_animation::BoardAnimator>,
                 mut panel: ResMut<new_menu::NewMenuPanel>| {
                    spawned.0 = false;
                    fl.0 = false;
                    *anim = board_animation::BoardAnimator::default();
                    *panel = new_menu::NewMenuPanel::default();
                },
            )
            .init_resource::<BrandLogoState>()
            .init_resource::<SolanaLogoState>()
            .init_resource::<PlayerColorChoice>()
            .init_resource::<NewsBannerState>()
            .insert_resource(PlayerIdentity::from_env())
            .init_resource::<crate::assets::GameAssets>()
            .init_resource::<crate::assets::LoadingProgress>()
            .init_resource::<crate::assets::AssetLoadingTimer>()
            .init_resource::<CompetitiveMenuState>()
            .init_resource::<P2PHostState>()
            .init_resource::<crate::states::tournament_menu::TournamentLobbyState>()
            .add_systems(
                EguiPrimaryContextPass,
                (
                    main_menu_ui_wrapper.run_if(in_state(GameState::MainMenu)),
                    render_lobby_selection_popup
                        .run_if(in_state(crate::core::MenuState::LobbySelection))
                        .run_if(in_state(GameState::MainMenu)),
                ),
            )
            .add_systems(
                Update,
                (
                    check_asset_loading,
                    handle_asset_loading_errors,
                    handle_untyped_asset_loading_errors,
                    ensure_menu_camera_setup,
                    sync_player_identity_from_wallet,
                    poll_wallet_bridge,
                    orbit_camera_system,
                    spawn_menu_bg_pieces,
                    try_setup_fonts,
                    board_animation::init_board_animator,
                    board_animation::animate_board_system,
                    board_animation::animate_menu_pieces,
                )
                    .run_if(in_state(GameState::MainMenu)),
            );
    }
}

/// Wrapper for [`main_menu_ui`] that surfaces query-single errors as warnings.
fn main_menu_ui_wrapper(mut ctx: MainMenuUIContext) {
    match main_menu_ui(&mut ctx) {
        Ok(()) => {}
        Err(e) => {
            warn!("[MAIN_MENU] UI system error: {:?}", e);
        }
    }
}

/// Marker component for the menu camera.
#[derive(Component)]
struct MenuCamera;

/// Resource to track the player's chosen color when playing vs AI.
#[derive(Resource)]
pub struct PlayerColorChoice {
    pub play_as_white: bool,
    pub selected: bool,
}

impl Default for PlayerColorChoice {
    fn default() -> Self {
        Self { play_as_white: true, selected: true }
    }
}

/// Filter controlling which lobby listings are shown on the home page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LobbyFilter {
    #[default]
    All,
    Free,
    Wagered,
}

#[derive(Resource)]
pub struct CompetitiveMenuState {
    /// Which game-type filter is selected in the lobby browser.
    pub lobby_filter: LobbyFilter,
    /// Whether the AI setup modal is currently open.
    pub show_ai_setup: bool,
    /// Selected AI difficulty level (1-8).
    pub ai_difficulty: u8,
    /// Selected player side (Black, Random, White).
    pub ai_side: AISide,
    /// Selected time control for AI games.
    pub ai_time_control: crate::game::time_control::TimeControl,
    /// Whether the spectator popup is currently open.
    pub show_spectator_popup: bool,
    /// Whether the controls popup is currently open.
    pub show_controls_popup: bool,
    /// Whether the join lobby popup is currently open.
    pub show_join_popup: bool,
    /// Input field for game ID to join in the join lobby popup.
    pub join_game_id: String,
    /// Selected AI engine (Stockfish or XFChessEngine).
    pub ai_engine: crate::game::ai::resource::AIEngine,
}

impl Default for CompetitiveMenuState {
    fn default() -> Self {
        Self {
            lobby_filter: LobbyFilter::default(),
            show_ai_setup: false,
            ai_difficulty: 4,
            ai_side: AISide::default(),
            ai_time_control: crate::game::time_control::TimeControl::Blitz,
            show_spectator_popup: false,
            show_controls_popup: false,
            show_join_popup: false,
            join_game_id: String::new(),
            ai_engine: crate::game::ai::resource::AIEngine::Stockfish,
        }
    }
}

/// Player side selection for AI games.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AISide {
    Black,
    Random,
    White,
}

impl Default for AISide {
    fn default() -> Self {
        Self::Random
    }
}

/// State for configuring a P2P game before hosting.
#[derive(Resource, Debug, Clone, Reflect)]
#[reflect(Resource)]
pub struct P2PHostState {
    /// Base time in minutes.
    pub base_time_minutes: u32,
    /// Increment in seconds.
    pub increment_seconds: u16,
    /// Stake amount (for P2P, usually 0 unless linked to Solana).
    pub stake_amount: f64,
    /// The generated game ID.
    pub game_id: Option<String>,
    /// Tracks when we last sent a heartbeat to keep the lobby alive.
    pub last_heartbeat: Option<std::time::Instant>,
    /// Optional room name displayed in the lobby browser.
    pub lobby_name: String,
}

impl Default for P2PHostState {
    fn default() -> Self {
        Self {
            base_time_minutes: 10,
            increment_seconds: 5,
            stake_amount: 0.0,
            game_id: None,
            last_heartbeat: None,
            lobby_name: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Wallet bridge polling — syncs PlayerIdentity from the Tauri HTTP bridge
// ---------------------------------------------------------------------------

/// Shared state populated by the background wallet bridge poller.
#[derive(Default, Clone)]
pub struct WalletBridgeData {
    pub sol_balance: f64,
    pub usd_balance: Option<f64>,
}

/// Polling resource for the Tauri wallet bridge at http://localhost:7454/status.
#[derive(Resource, Default)]
pub struct WalletBridgePoller {
    /// Channel for incoming (pubkey, username) from a `/status` poll.
    pub status_rx: Option<crossbeam_channel::Receiver<(Option<String>, Option<String>)>>,
    /// Channel for incoming (sol, usd) balance.
    pub balance_rx: Option<crossbeam_channel::Receiver<(f64, f64)>>,
    /// Seconds since last poll trigger.
    pub timer: f32,
    /// Last known pubkey — used to detect new connections.
    pub known_pubkey: Option<String>,
    /// Shared balance data exposed to the UI via `MainMenuUIContext`.
    pub data: std::sync::Arc<std::sync::Mutex<WalletBridgeData>>,
}

fn poll_wallet_bridge(
    mut poller: ResMut<WalletBridgePoller>,
    mut player_identity: ResMut<PlayerIdentity>,
    time: Res<Time>,
) {
    // --- receive status response ---
    if let Some(ref rx) = poller.status_rx {
        if let Ok((pubkey_opt, username_opt)) = rx.try_recv() {
            poller.status_rx = None;
            if let Some(pk) = pubkey_opt {
                // Always update username from bridge if we have one
                if let Some(ref uname) = username_opt {
                    if player_identity.username.as_deref() != Some(uname.as_str()) {
                        info!("[WalletBridge] Username from bridge: {}", uname);
                        player_identity.username = Some(uname.clone());
                    }
                }

                if poller.known_pubkey.as_deref() != Some(&pk) {
                    info!("[WalletBridge] New pubkey detected: {}", pk);
                    poller.known_pubkey = Some(pk.clone());

                    // If bridge didn't supply a username, fall back to VPS profile fetch
                    if username_opt.is_none() || username_opt.as_deref() == Some("") {
                        let (tx, rx) = crossbeam_channel::bounded(1);
                        player_identity.pending_profile_rx = Some(rx);
                        let pk2 = pk.clone();
                        bevy::tasks::IoTaskPool::get().spawn(async move {
                            let res = crate::multiplayer::network::vps::identity::fetch_player_profile(&pk2);
                            let _ = tx.send(res);
                        }).detach();
                    }

                    // Trigger balance fetch
                    let (btx, brx) = crossbeam_channel::bounded(1);
                    poller.balance_rx = Some(brx);
                    let data_arc = poller.data.clone();
                    bevy::tasks::IoTaskPool::get().spawn(async move {
                        let (sol, usd) = fetch_sol_balance_usd(&pk);
                        let _ = btx.send((sol, usd));
                        let mut d = data_arc.lock().unwrap();
                        d.sol_balance = sol;
                        d.usd_balance = Some(usd);
                    }).detach();
                }
            }
        }
    }

    // --- receive balance response ---
    if let Some(ref rx) = poller.balance_rx {
        if let Ok((sol, usd)) = rx.try_recv() {
            poller.balance_rx = None;
            let mut d = poller.data.lock().unwrap();
            d.sol_balance = sol;
            d.usd_balance = Some(usd);
        }
    }

    // --- poll every 5 seconds ---
    poller.timer += time.delta_secs();
    if poller.timer >= 5.0 && poller.status_rx.is_none() {
        poller.timer = 0.0;
        let (tx, rx) = crossbeam_channel::bounded(1);
        poller.status_rx = Some(rx);
        bevy::tasks::IoTaskPool::get().spawn(async move {
            let result = fetch_bridge_status();
            let _ = tx.send(result);
        }).detach();
    }
}

/// GET http://localhost:7454/status and extract pubkey + username.
fn fetch_bridge_status() -> (Option<String>, Option<String>) {
    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
    {
        Ok(c) => c,
        Err(_) => return (None, None),
    };
    let port = std::env::var("XFCHESS_WALLET_PORT").ok()
        .and_then(|v| v.parse::<u16>().ok()).unwrap_or(7454);
    let resp = match client.get(format!("http://127.0.0.1:{port}/status")).send() {
        Ok(r) => r,
        Err(_) => return (None, None),
    };
    let json: serde_json::Value = match resp.json() {
        Ok(v) => v,
        Err(_) => return (None, None),
    };
    let pubkey = json["pubkey"].as_str().map(|s| s.to_string());
    let username = json["username"].as_str()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    (pubkey, username)
}

/// Fetch SOL balance via Helius RPC getBalance, then convert to USD via
/// the backend rate endpoint. Returns (sol, usd) — both 0.0 on any error.
fn fetch_sol_balance_usd(pubkey: &str) -> (f64, f64) {
    let rpc_url = format!(
        "https://beta.helius-rpc.com/?api-key=5bb5fed2-8d33-458b-b7d2-3d18fdbb3da5"
    );

    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
    {
        Ok(c) => c,
        Err(_) => return (0.0, 0.0),
    };

    // getBalance RPC call
    let body = serde_json::json!({
        "jsonrpc": "2.0", "id": 1,
        "method": "getBalance",
        "params": [pubkey, { "commitment": "confirmed" }]
    });
    let sol = client.post(&rpc_url).json(&body).send()
        .ok()
        .and_then(|r| r.json::<serde_json::Value>().ok())
        .and_then(|j| j["result"]["value"].as_u64())
        .map(|lamports| lamports as f64 / 1_000_000_000.0)
        .unwrap_or(0.0);

    if sol == 0.0 {
        return (0.0, 0.0);
    }

    // Fetch SOL/USD rate from local backend or CoinGecko fallback
    let usd_per_sol = client
        .get("http://127.0.0.1:8090/api/rates/all")
        .send()
        .ok()
        .and_then(|r| r.json::<serde_json::Value>().ok())
        .and_then(|j| j["rates"]["usd"].as_f64())
        .or_else(|| {
            client.get("https://api.coingecko.com/api/v3/simple/price?ids=solana&vs_currencies=usd")
                .send().ok()
                .and_then(|r| r.json::<serde_json::Value>().ok())
                .and_then(|j| j["solana"]["usd"].as_f64())
        })
        .unwrap_or(0.0);

    (sol, sol * usd_per_sol)
}

/// Logged-in player identity passed to the game by the Tauri wallet UI or
/// the web profile deep-link (via the `XFCHESS_USERNAME` env var).
#[derive(Resource, Debug, Clone, Default)]
pub struct PlayerIdentity {
    pub username: Option<String>,
    /// Cached ELO rating from VPS backend
    pub elo: Option<u32>,
    /// Cached country code from VPS backend
    pub country: Option<String>,
    /// Receiver for an in-flight profile fetch from VPS
    pub pending_profile_rx: Option<crossbeam_channel::Receiver<Result<crate::multiplayer::network::vps::identity::PlayerProfile, String>>>,
}

impl PlayerIdentity {
    /// Read the username from the `XFCHESS_USERNAME` env var, if set and non-empty.
    pub fn from_env() -> Self {
        let username = std::env::var("XFCHESS_USERNAME")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        Self {
            username,
            ..Default::default()
        }
    }

    /// Display label used in the menu UI.
    pub fn display_name(&self) -> &str {
        self.username.as_deref().unwrap_or("Guest")
    }

    /// Returns the ELO string for display, e.g. "1420" or "—".
    pub fn display_elo(&self) -> String {
        self.elo.map(|e| e.to_string()).unwrap_or_else(|| "—".to_string())
    }
}

/// Cached NEWS banner texture loaded from the local screenshot file.
#[derive(Resource, Default)]
pub struct NewsBannerState {
    pub texture: Option<egui::TextureHandle>,
    pub loaded: bool,
}

const NEWS_BANNER_PATH: &str = r"C:\Users\isich\Pictures\Camera Roll\Screenshots\Screenshot 2026-04-08 172321.png";

/// Cached Solana splash textures (Screenshot logo + Solana coin logo).
#[derive(Resource, Default)]
pub struct SolanaLogoState {
    pub texture1: Option<egui::TextureHandle>,
    pub texture2: Option<egui::TextureHandle>,
    pub loaded: bool,
}

const SOLANA_LOGO1_PATH: &str =
    r"C:\Users\isich\Pictures\Camera Roll\Screenshots\Screenshot 2026-05-20 211643.png";
const SOLANA_LOGO2_PATH: &str =
    r"C:\Users\isich\Downloads\solanaLogo.png";

pub(super) fn ensure_solana_logos(ctx: &egui::Context, logos: &mut SolanaLogoState) {
    if logos.loaded {
        return;
    }
    logos.loaded = true;

    const MAX_SIDE: u32 = 2048;

    let load = |path: &str| -> Option<egui::ColorImage> {
        let bytes = std::fs::read(path).ok()?;
        let img = image::load_from_memory(&bytes).ok()?;
        // Scale down if either dimension exceeds egui's texture limit
        let img = if img.width() > MAX_SIDE || img.height() > MAX_SIDE {
            let scale = (MAX_SIDE as f32 / img.width().max(img.height()) as f32).min(1.0);
            let nw = ((img.width() as f32 * scale) as u32).max(1);
            let nh = ((img.height() as f32 * scale) as u32).max(1);
            img.resize(nw, nh, image::imageops::FilterType::Lanczos3)
        } else {
            img
        };
        let rgba = img.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        Some(egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
    };

    if let Some(ci) = load(SOLANA_LOGO1_PATH) {
        logos.texture1 = Some(ctx.load_texture("solana_logo1", ci, egui::TextureOptions::LINEAR));
    }
    if let Some(ci) = load(SOLANA_LOGO2_PATH) {
        logos.texture2 = Some(ctx.load_texture("solana_logo2", ci, egui::TextureOptions::LINEAR));
    }
}

/// Cached brand logo texture loaded from the local screenshot file.
#[derive(Resource, Default)]
pub struct BrandLogoState {
    pub texture: Option<egui::TextureHandle>,
    pub loaded: bool,
}

const BRAND_LOGO_PATH: &str = r"C:\Users\isich\Pictures\Camera Roll\Screenshots\Screenshot 2026-04-22 232508.png";

/// Grey bezel border color shared by popups and modals.
const BEZEL_GREY: egui::Color32 = egui::Color32::from_rgb(100, 100, 100);

fn ensure_news_banner_texture(ctx: &egui::Context, banner: &mut NewsBannerState) -> Option<egui::TextureId> {
    if let Some(texture) = banner.texture.as_ref() {
        return Some(texture.id());
    }
    if banner.loaded {
        return None;
    }
    banner.loaded = true;

    let Ok(bytes) = std::fs::read(NEWS_BANNER_PATH) else {
        warn!("[MAIN_MENU] Failed to read news banner image at {}", NEWS_BANNER_PATH);
        return None;
    };
    let Ok(decoded) = image::load_from_memory(&bytes) else {
        warn!("[MAIN_MENU] Failed to decode news banner image at {}", NEWS_BANNER_PATH);
        return None;
    };

    let rgba = decoded.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    let texture = ctx.load_texture("news_banner_screenshot", color_image, egui::TextureOptions::LINEAR);
    let texture_id = texture.id();
    banner.texture = Some(texture);
    Some(texture_id)
}

fn ensure_brand_logo_texture(ctx: &egui::Context, logo: &mut BrandLogoState) -> Option<egui::TextureId> {
    if let Some(texture) = logo.texture.as_ref() {
        return Some(texture.id());
    }
    if logo.loaded {
        return None;
    }
    logo.loaded = true;

    let Ok(bytes) = std::fs::read(BRAND_LOGO_PATH) else {
        warn!("[MAIN_MENU] Failed to read brand logo image at {}", BRAND_LOGO_PATH);
        return None;
    };
    let Ok(decoded) = image::load_from_memory(&bytes) else {
        warn!("[MAIN_MENU] Failed to decode brand logo image at {}", BRAND_LOGO_PATH);
        return None;
    };

    let rgba = decoded.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    let texture = ctx.load_texture("brand_logo_screenshot", color_image, egui::TextureOptions::LINEAR);
    let texture_id = texture.id();
    logo.texture = Some(texture);
    Some(texture_id)
}

/// Setup the main menu camera (the persistent egui camera is reused).
fn setup_menu_camera(
    mut commands: Commands,
    persistent_camera: Res<crate::PersistentEguiCamera>,
    mut camera_query: Query<
        &mut Transform,
        (With<Camera3d>, Without<MenuCamera>),
    >,
) {
    debug!("[MAIN_MENU] Setting up menu camera for egui launcher");

    if let Some(camera_entity) = persistent_camera.entity {
        match camera_query.get_mut(camera_entity) {
            Ok(mut transform) => {
                info!("[MAIN_MENU] Setting up menu camera entity: {:?}", camera_entity);
                *transform = Transform::from_translation(new_menu::BOARD_CAM)
                    .looking_at(new_menu::BOARD_CENTER, Vec3::Y);
                commands.entity(camera_entity).insert(MenuCamera);
            }
            Err(e) => {
                error!(
                    "[MAIN_MENU] ERROR: Persistent camera entity {:?} exists but query failed: {:?}",
                    camera_entity, e
                );
            }
        }
    } else {
        debug!("[MAIN_MENU] WARNING: Persistent camera not yet created (OnEnter runs before PreStartup).");
    }
}

/// Late-init fallback in case `OnEnter(MainMenu)` ran before the persistent
/// camera was created (which can happen for the default state).
fn ensure_menu_camera_setup(
    persistent_camera: Res<crate::PersistentEguiCamera>,
    mut camera_query: Query<
        &mut Transform,
        (With<Camera3d>, Without<MenuCamera>),
    >,
    mut commands: Commands,
    menu_camera_query: Query<Entity, With<MenuCamera>>,
) {
    if menu_camera_query.is_empty() {
        if let Some(camera_entity) = persistent_camera.entity {
            match camera_query.get_mut(camera_entity) {
                Ok(mut transform) => {
                    info!("[MAIN_MENU] Initializing menu camera (late setup)");
                    *transform = Transform::from_translation(new_menu::BOARD_CAM)
                        .looking_at(new_menu::BOARD_CENTER, Vec3::Y);
                    commands.entity(camera_entity).insert(MenuCamera);
                }
                Err(e) => {
                    error!(
                        "[MAIN_MENU] ERROR: Late setup failed to query camera: {:?}",
                        e
                    );
                }
            }
        }
    }
}

/// Main menu UI orchestrator. Dispatches to a screen-specific renderer based
/// on the current `MenuState`, otherwise falls back to the website-style menu.
fn main_menu_ui(ctx: &mut MainMenuUIContext) -> Result<(), bevy::ecs::query::QuerySingleError> {
    let current_substate = if let Some(ref menu_state_res) = ctx.current_menu_state {
        *menu_state_res.get()
    } else {
        crate::core::MenuState::Main
    };

    let egui_ctx = ctx.contexts.ctx_mut()?.clone();

    // Always render the main menu (keeps 3D board visible behind all popups)
    render_website_menu(&egui_ctx, ctx);

    let popup_frame = egui::Frame {
        fill: egui::Color32::from_rgba_unmultiplied(18, 18, 22, 242),
        inner_margin: egui::Margin::same(20),
        outer_margin: egui::Margin::ZERO,
        corner_radius: egui::CornerRadius::same(8),
        stroke: egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(80, 80, 100, 180)),
        shadow: egui::Shadow { blur: 24, spread: 4, color: egui::Color32::from_black_alpha(180), offset: [0, 4] },
    };

    if current_substate == crate::core::MenuState::BraidLobby {
        egui::Window::new("p2p_lobby_popup")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([420.0, 480.0])
            .frame(popup_frame)
            .show(&egui_ctx, |ui| {
                render_braid_lobby_screen(ui, ctx);
            });
    }

    if current_substate == crate::core::MenuState::Tournaments {
        egui::Window::new("tournaments_popup")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([480.0, 520.0])
            .frame(popup_frame)
            .show(&egui_ctx, |ui| {
                render_tournament_browser_screen(ui, ctx);
            });
    }

    if current_substate == crate::core::MenuState::HostConfig {
        egui::Window::new("host_config_popup")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([380.0, 320.0])
            .frame(popup_frame)
            .show(&egui_ctx, |ui| {
                render_host_p2p_config_screen(ui, ctx);
            });
    }

    if current_substate == crate::core::MenuState::P2PWaiting {
        egui::Window::new("p2p_waiting_popup")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([340.0, 260.0])
            .frame(popup_frame)
            .show(&egui_ctx, |ui| {
                render_p2p_waiting_screen(ui, ctx);
            });
    }

    #[cfg(feature = "solana")]
    if current_substate == crate::core::MenuState::SolanaLobby {
        egui::Window::new("solana_lobby_popup")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([460.0, 500.0])
            .frame(popup_frame)
            .show(&egui_ctx, |ui| {
                ui_solana_lobby(ui, ctx);
            });
    }

    Ok(())
}

/// Setup custom fonts for the main menu.
///
/// Tries multiple locations in order:
/// 1. Project `assets/fonts/` (development)
/// 2. Executable directory (bundled app)
/// 3. System fallback (uses default egui font)
/// Tracks whether egui fonts have been registered (retried each frame until success).
#[derive(Resource, Default)]
struct FontsLoaded(bool);

/// Runs every Update frame until it successfully gets the egui context, then
/// registers Cinzel + OpenSans and sets `FontsLoaded`. Safe to call repeatedly.
fn try_setup_fonts(mut contexts: EguiContexts, mut loaded: ResMut<FontsLoaded>) {
    if loaded.0 {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return; // egui not ready yet — retry next frame
    };

    let mut fonts = egui::FontDefinitions::default();

    // Load Cinzel (title / heading font — classical serif)
    let cinzel_paths = [
        "assets/fonts/Cinzel-Regular.ttf",
        "./assets/fonts/Cinzel-Regular.ttf",
    ];
    for path in &cinzel_paths {
        if let Ok(data) = std::fs::read(path) {
            fonts.font_data.insert("Cinzel".to_owned(), Arc::new(egui::FontData::from_owned(data)));
            info!("[MAIN_MENU] Loaded Cinzel font from {}", path);
            break;
        }
    }

    // Load Cinzel Bold
    let cinzel_bold_paths = [
        "assets/fonts/Cinzel-Bold.ttf",
        "./assets/fonts/Cinzel-Bold.ttf",
    ];
    for path in &cinzel_bold_paths {
        if let Ok(data) = std::fs::read(path) {
            fonts.font_data.insert("CinzelBold".to_owned(), Arc::new(egui::FontData::from_owned(data)));
            info!("[MAIN_MENU] Loaded Cinzel Bold font from {}", path);
            break;
        }
    }

    // Load OpenSans as the body/proportional fallback
    let opensans_paths = [
        "assets/fonts/OpenSans-VariableFont_wdth,wght.ttf",
        "./assets/fonts/OpenSans-VariableFont_wdth,wght.ttf",
    ];
    let mut body_loaded = false;
    for path in &opensans_paths {
        if let Ok(data) = std::fs::read(path) {
            fonts.font_data.insert("OpenSans".to_owned(), Arc::new(egui::FontData::from_owned(data)));
            info!("[MAIN_MENU] Loaded OpenSans font from {}", path);
            body_loaded = true;
            break;
        }
    }

    // Proportional family: Cinzel first (gives egui::FontFamily::Proportional a Cinzel default),
    // then OpenSans as fallback for characters Cinzel doesn't cover.
    let proportional = fonts.families.entry(egui::FontFamily::Proportional).or_default();
    if fonts.font_data.contains_key("Cinzel") {
        proportional.insert(0, "Cinzel".to_owned());
    }
    if body_loaded {
        // OpenSans sits after Cinzel for fallback coverage
        let pos = if fonts.font_data.contains_key("Cinzel") { 1 } else { 0 };
        proportional.insert(pos, "OpenSans".to_owned());
    }

    // Register a named family so UI code can request Cinzel explicitly
    if fonts.font_data.contains_key("Cinzel") {
        fonts.families.insert(
            egui::FontFamily::Name("Cinzel".into()),
            vec!["Cinzel".to_owned()],
        );
    }
    if fonts.font_data.contains_key("CinzelBold") {
        fonts.families.insert(
            egui::FontFamily::Name("CinzelBold".into()),
            vec!["CinzelBold".to_owned()],
        );
    }

    ctx.set_fonts(fonts);
    loaded.0 = true;
    info!("[MAIN_MENU] Fonts registered successfully");
}

/// Render the main menu — dispatches to new-style (3D board) or classic website layout
/// depending on the current [`MenuStyle`] resource.
fn render_website_menu(ctx: &egui::Context, ctx_menu: &mut MainMenuUIContext) {
    if !ctx_menu.loading_progress.complete {
        render_loading_screen_website(ctx, ctx_menu);
        return;
    }

    if *ctx_menu.new_menu_panel == NewMenuPanel::SolanaMultiplayer {
        render_solana_splash(ctx, ctx_menu);
    } else {
        render_new_style_panel(ctx, ctx_menu);
    }

    render_wallet_hud(ctx, ctx_menu);

    if ctx_menu.competitive_menu.show_ai_setup {
        render_ai_setup_modal(
            ctx,
            &mut ctx_menu.competitive_menu,
            &mut ctx_menu.ai_config,
            &mut ctx_menu.core_mode,
            &mut ctx_menu.next_state,
            &mut ctx_menu.active_time_control,
        );
    }

    if ctx_menu.competitive_menu.show_spectator_popup {
        let cached_games = if let Some(vps) = &ctx_menu.p2p_vps_state {
            vps.cached_games.clone()
        } else {
            Vec::new()
        };
        render_spectator_popup(ctx, &mut ctx_menu.competitive_menu, &cached_games);
    }

    if ctx_menu.competitive_menu.show_controls_popup {
        render_controls_popup(ctx, &mut ctx_menu.competitive_menu);
    }

}

/// Sync `PlayerIdentity` with the on-chain wallet profile when a wallet is connected.
#[cfg(feature = "solana")]
fn sync_player_identity_from_wallet(
    mut player_identity: ResMut<PlayerIdentity>,
    solana_state: Option<Res<crate::multiplayer::solana::integration::state::SolanaIntegrationState>>,
) {
    let Some(ref solana_state) = solana_state else { return };
    let Some(pubkey) = solana_state.wallet_pubkey else { return };

    // Check if we already have a pending fetch
    if let Some(ref rx) = player_identity.pending_profile_rx {
        match rx.try_recv() {
            Ok(Ok(profile)) => {
                player_identity.username = Some(profile.username.clone());
                player_identity.elo = Some(profile.elo);
                player_identity.country = Some(profile.country);
                info!("[PROFILE] Fetched profile for {} — ELO {}", profile.username, profile.elo);
                player_identity.pending_profile_rx = None;
            }
            Ok(Err(e)) => {
                warn!("[PROFILE] Failed to fetch player profile: {}", e);
                player_identity.pending_profile_rx = None;
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {}
            Err(_) => {
                player_identity.pending_profile_rx = None;
            }
        }
        return;
    }

    // If we already have ELO cached, don't refetch
    if player_identity.elo.is_some() {
        return;
    }

    let pk = pubkey.to_string();
    let (tx, rx) = crossbeam_channel::bounded(1);
    player_identity.pending_profile_rx = Some(rx);

    bevy::tasks::IoTaskPool::get().spawn(async move {
        let res = crate::multiplayer::network::vps::identity::fetch_player_profile(&pk);
        let _ = tx.send(res);
    }).detach();
}

#[cfg(not(feature = "solana"))]
fn sync_player_identity_from_wallet(_: ResMut<PlayerIdentity>) {}
