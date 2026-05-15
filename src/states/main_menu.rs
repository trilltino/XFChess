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
#[path = "main_menu/navbar.rs"]
mod navbar;
#[path = "main_menu/sections.rs"]
mod sections;
#[path = "main_menu/modals.rs"]
mod modals;

use screens::*;
use navbar::render_navbar;
use sections::{
    render_learn_section, render_news_section, render_play_computer_section,
    render_quick_pairing_section, render_tournaments_box, render_updates_box,
};
use modals::{render_ai_setup_modal, render_controls_popup};

/// Plugin for main menu state.
pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
                OnEnter(GameState::MainMenu),
                (
                    setup_menu_camera,
                    start_asset_loading,
                    setup_custom_fonts,
                ),
            )
            .init_resource::<BrandLogoState>()
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
                )
                    .run_if(in_state(GameState::MainMenu))
                    .run_if(not(in_state(crate::core::MenuState::PieceViewer))),
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
}

impl Default for P2PHostState {
    fn default() -> Self {
        Self {
            base_time_minutes: 10,
            increment_seconds: 5,
            stake_amount: 0.0,
            game_id: None,
        }
    }
}

/// Logged-in player identity passed to the game by the Tauri wallet UI or
/// the web profile deep-link (via the `XFCHESS_USERNAME` env var).
#[derive(Resource, Debug, Clone, Default)]
pub struct PlayerIdentity {
    pub username: Option<String>,
}

impl PlayerIdentity {
    /// Read the username from the `XFCHESS_USERNAME` env var, if set and non-empty.
    pub fn from_env() -> Self {
        let username = std::env::var("XFCHESS_USERNAME")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        Self { username }
    }

    /// Display label used in the menu UI.
    pub fn display_name(&self) -> &str {
        self.username.as_deref().unwrap_or("Guest")
    }
}

/// Cached NEWS banner texture loaded from the local screenshot file.
#[derive(Resource, Default)]
pub struct NewsBannerState {
    pub texture: Option<egui::TextureHandle>,
    pub loaded: bool,
}

const NEWS_BANNER_PATH: &str = r"C:\Users\isich\Pictures\Camera Roll\Screenshots\Screenshot 2026-04-08 172321.png";

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
                *transform = Transform::from_xyz(0.0, 0.0, 1.0);
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
                    *transform = Transform::from_xyz(0.0, 0.0, 1.0);
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

    if current_substate == crate::core::MenuState::PieceViewer {
        return Ok(());
    }

    let egui_ctx = ctx.contexts.ctx_mut()?.clone();

    #[cfg(feature = "solana")]
    if current_substate == crate::core::MenuState::SolanaLobby {
        egui::CentralPanel::default()
            .frame(egui::Frame {
                fill: egui::Color32::from_rgba_unmultiplied(45, 45, 45, 220),
                inner_margin: egui::Margin::same(20),
                ..egui::Frame::NONE
            })
            .show(&egui_ctx, |ui| {
                ui_solana_lobby(ui, ctx);
            });
        return Ok(());
    }

    if current_substate == crate::core::MenuState::BraidLobby {
        egui::CentralPanel::default()
            .frame(egui::Frame {
                fill: egui::Color32::from_rgba_unmultiplied(45, 45, 45, 220),
                inner_margin: egui::Margin::same(20),
                ..egui::Frame::NONE
            })
            .show(&egui_ctx, |ui| {
                render_braid_lobby_screen(ui, ctx);
            });
        return Ok(());
    }

    if current_substate == crate::core::MenuState::Tournaments {
        egui::CentralPanel::default()
            .frame(egui::Frame {
                fill: egui::Color32::from_rgba_unmultiplied(45, 45, 45, 220),
                inner_margin: egui::Margin::same(20),
                ..egui::Frame::NONE
            })
            .show(&egui_ctx, |ui| {
                render_tournament_browser_screen(ui, ctx);
            });
        return Ok(());
    }

    if current_substate == crate::core::MenuState::HostConfig {
        egui::CentralPanel::default()
            .frame(egui::Frame {
                fill: egui::Color32::from_rgba_unmultiplied(25, 25, 30, 240),
                inner_margin: egui::Margin::same(30),
                ..egui::Frame::NONE
            })
            .show(&egui_ctx, |ui| {
                render_host_p2p_config_screen(ui, ctx);
            });
        return Ok(());
    }

    if current_substate == crate::core::MenuState::P2PWaiting {
        egui::CentralPanel::default()
            .frame(egui::Frame {
                fill: egui::Color32::from_rgba_unmultiplied(25, 25, 30, 240),
                inner_margin: egui::Margin::same(30),
                ..egui::Frame::NONE
            })
            .show(&egui_ctx, |ui| {
                render_p2p_waiting_screen(ui, ctx);
            });
        return Ok(());
    }

    render_website_menu(&egui_ctx, ctx);
    Ok(())
}

/// Setup custom fonts for the main menu.
///
/// Tries multiple locations in order:
/// 1. Project `assets/fonts/` (development)
/// 2. Executable directory (bundled app)
/// 3. System fallback (uses default egui font)
fn setup_custom_fonts(mut contexts: EguiContexts) {
    let ctx_result = contexts.ctx_mut();
    let Ok(ctx) = ctx_result else {
        warn!("[MAIN_MENU] Failed to get egui context for font setup");
        return;
    };

    let mut fonts = egui::FontDefinitions::default();

    let possible_font_paths = [
        "assets/fonts/OpenSans-VariableFont_wdth,wght.ttf".to_string(),
        "./assets/fonts/OpenSans-VariableFont_wdth,wght.ttf".to_string(),
    ];

    let mut font_loaded = false;
    for font_path in &possible_font_paths {
        if let Ok(font_data) = std::fs::read(font_path) {
            fonts.font_data.insert(
                "OpenSans".to_owned(),
                Arc::new(egui::FontData::from_owned(font_data)),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "OpenSans".to_owned());

            ctx.set_fonts(fonts);
            info!("[MAIN_MENU] Loaded OpenSans font successfully from {}", font_path);
            font_loaded = true;
            break;
        }
    }

    if !font_loaded {
        warn!("[MAIN_MENU] Could not load OpenSans font, using egui default font");
    }
}

/// Render the website-style main menu (navbar + content grid + popups).
fn render_website_menu(ctx: &egui::Context, ctx_menu: &mut MainMenuUIContext) {
    if !ctx_menu.loading_progress.complete {
        render_loading_screen_website(ctx, ctx_menu);
        return;
    }

    render_navbar(ctx, ctx_menu);

    egui::CentralPanel::default()
        .frame(egui::Frame {
            fill: egui::Color32::from_rgba_unmultiplied(45, 45, 45, 210),
            ..egui::Frame::NONE
        })
        .show(ctx, |ui| {
            ui.add_space(50.0);

            egui::ScrollArea::vertical()
                .show(ui, |ui| {
                    // Two equal columns: PLAY | QUICK PLAY
                    let available_w = ui.available_width();
                    let col_spacing = 14.0;
                    let top_col_w = ((available_w - col_spacing) / 2.0).max(0.0);

                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(top_col_w);
                            render_play_computer_section(ui, ctx_menu);
                        });

                        ui.add_space(col_spacing);

                        ui.vertical(|ui| {
                            ui.set_width(top_col_w);
                            render_quick_pairing_section(ui, ctx_menu);
                        });
                    });

                    ui.add_space(24.0);

                    // NEWS & LEARN row — widths matched to a top column so the
                    // menu reads as a consistent grid.
                    let middle_col_spacing = col_spacing;
                    let box_width = top_col_w;
                    let side_margin =
                        ((available_w - middle_col_spacing - (box_width * 2.0)) * 0.5).max(0.0);

                    ui.horizontal(|ui| {
                        ui.add_space(side_margin);

                        ui.vertical(|ui| {
                            ui.set_width(box_width);
                            render_news_section(ui, box_width, &mut ctx_menu.news_banner);
                        });

                        ui.add_space(middle_col_spacing);

                        ui.vertical(|ui| {
                            ui.set_width(box_width);
                            render_learn_section(ui, box_width, &mut ctx_menu.learn_viewport);
                        });

                        ui.add_space(side_margin);
                    });

                    ui.add_space(30.0);

                    // Bottom row: TOURNAMENTS & UPDATES
                    let bottom_box_height = 250.0;
                    let bottom_col_spacing = col_spacing;
                    let bottom_col_w = ((available_w - bottom_col_spacing) * 0.5).max(0.0);

                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(bottom_col_w);
                            render_tournaments_box(ui, ctx_menu);
                        });

                        ui.add_space(bottom_col_spacing);

                        ui.vertical(|ui| {
                            ui.set_width(bottom_col_w);
                            render_updates_box(ui, bottom_box_height);
                        });
                    });
                });
        });

    // AI SETUP MODAL
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

    // SPECTATOR POPUP
    if ctx_menu.competitive_menu.show_spectator_popup {
        let cached_games = if let Some(vps) = &ctx_menu.p2p_vps_state {
            vps.cached_games.clone()
        } else {
            Vec::new()
        };
        render_spectator_popup(ctx, &mut ctx_menu.competitive_menu, &cached_games);
    }

    // CONTROLS POPUP
    if ctx_menu.competitive_menu.show_controls_popup {
        render_controls_popup(ctx, &mut ctx_menu.competitive_menu);
    }

    // JOIN LOBBY POPUP
    if ctx_menu.competitive_menu.show_join_popup {
        render_join_lobby_popup(ctx, ctx_menu);
    }
}
