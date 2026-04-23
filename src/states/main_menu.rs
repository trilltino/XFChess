#![allow(dead_code)]
//! Main menu plugin with polished UI
//!
//! Displays the primary game menu with options to:
//! - Start a new game (with mode selection)
//! - Access settings
//! - View statistics
//! - Exit the application
//!
//! Features styled UI components from the theme system and
//! an optional animated 3D background scene.

use crate::assets::{
    check_asset_loading, handle_asset_loading_errors, handle_untyped_asset_loading_errors,
    start_asset_loading,
};
use crate::core::{GameMode as CoreGameMode, GameState};
use crate::game::ai::GameMode;
use rand;
use crate::xf_animate::XfAnimatePlugin;
#[cfg(feature = "solana")]
use crate::multiplayer::solana::lobby::{
    spawn_create_game, spawn_join_game, spawn_lookup_game, spawn_poll_opponent_joined,
    LobbyMode, LobbyStatus,
};
use crate::ui::styles::{Layout, *};
use crate::ui::system_params::MainMenuUIContext;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use std::sync::Arc;

/// Plugin for main menu state
pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(XfAnimatePlugin)
            .add_systems(
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

/// Wrapper for main_menu_ui that handles Result
fn main_menu_ui_wrapper(mut ctx: MainMenuUIContext) {
    match main_menu_ui(&mut ctx) {
        Ok(()) => {
            // UI rendered successfully
        }
        Err(e) => {
            warn!("[MAIN_MENU] UI system error: {:?}", e);
        }
    }
}

/// Marker component for menu camera
#[derive(Component)]
struct MenuCamera;

/// Resource to track the player's chosen color when playing vs AI
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

#[derive(Resource, Default)]
pub struct CompetitiveMenuState {
    /// Which game-type filter is selected in the lobby browser.
    pub lobby_filter: LobbyFilter,
    /// Whether the AI setup modal is currently open
    pub show_ai_setup: bool,
    /// Selected AI difficulty level (1-8)
    pub ai_difficulty: u8,
    /// Selected player side (Black, Random, White)
    pub ai_side: AISide,
    /// Whether the spectator popup is currently open
    pub show_spectator_popup: bool,
    /// Whether the controls popup is currently open
    pub show_controls_popup: bool,
    /// Whether the join lobby popup is currently open
    pub show_join_popup: bool,
    /// Input field for game ID to join in the join lobby popup
    pub join_game_id: String,
}

/// Player side selection for AI games
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





/// Setup camera for main menu - simplified for egui-only launcher
fn setup_menu_camera(
    mut commands: Commands,
    persistent_camera: Res<crate::PersistentEguiCamera>,
    mut camera_query: Query<
        &mut Transform,
        (With<Camera3d>, Without<MenuCamera>),
    >,
) {
    debug!("[MAIN_MENU] Setting up menu camera for egui launcher");

    // Update persistent camera transform for menu view
    // Handle gracefully if camera doesn't exist yet (OnEnter runs before PreStartup for default state)
    if let Some(camera_entity) = persistent_camera.entity {
        match camera_query.get_mut(camera_entity) {
            Ok(mut transform) => {
                info!("[MAIN_MENU] Setting up menu camera entity: {:?}", camera_entity);
                // Simple default transform for egui-only menu
                *transform = Transform::from_xyz(0.0, 0.0, 1.0);
                debug!("[MAIN_MENU] Updated persistent camera transform for egui menu");

                // Add menu marker to persistent camera
                commands.entity(camera_entity).insert(MenuCamera);
                debug!("[MAIN_MENU] Menu camera setup complete");
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



/// Ensure menu camera is set up if it wasn't ready during OnEnter
/// This handles the case where OnEnter runs before PreStartup (for default state)
fn ensure_menu_camera_setup(
    persistent_camera: Res<crate::PersistentEguiCamera>,
    mut camera_query: Query<
        &mut Transform,
        (With<Camera3d>, Without<MenuCamera>),
    >,
    mut commands: Commands,
    menu_camera_query: Query<Entity, With<MenuCamera>>,
) {
    // Only set up if camera exists and menu camera marker is not present
    if menu_camera_query.is_empty() {
        if let Some(camera_entity) = persistent_camera.entity {
            match camera_query.get_mut(camera_entity) {
                Ok(mut transform) => {
                    info!("[MAIN_MENU] Initializing menu camera (late setup)");
                    // Simple default transform for egui-only menu
                    *transform = Transform::from_xyz(0.0, 0.0, 1.0);
                    debug!("[MAIN_MENU] Late setup: Updated persistent camera transform for egui menu");

                    // Add menu marker to persistent camera
                    commands.entity(camera_entity).insert(MenuCamera);
                    debug!("[MAIN_MENU] Late camera setup completed successfully");
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


/// Main menu UI system
fn main_menu_ui(ctx: &mut MainMenuUIContext) -> Result<(), bevy::ecs::query::QuerySingleError> {
    // Only show main menu UI when in a known MenuState (not PieceViewer which has its own system usually,
    // but assuming PieceViewer is a substate of MainMenu, so it might need its own UI or exit button.
    // For now we handle Main, ModeSelect, About.

    // Check if we are in a valid substate
    let current_substate = if let Some(ref menu_state_res) = ctx.current_menu_state {
        *menu_state_res.get()
    } else {
        // Default to Main if state not found (shouldn't happen)
        crate::core::MenuState::Main
    };

    // If in PieceViewer, return early (assuming separate system handles it, or just "back" button needed)
    // The previous code returned if not Main. Now we want to handle ModeSelect and About too.
    if current_substate == crate::core::MenuState::PieceViewer {
        // Maybe render a "Back" button overlay for PieceViewer?
        // For now, let's stick to the core task: Main, ModeSelect, About.
        return Ok(());
    }

    // Clone the egui Context (Arc-backed, cheap) so we don't hold a mutable borrow
    // on ctx.contexts across the closures that also need &mut ctx.
    let egui_ctx = ctx.contexts.ctx_mut()?.clone();





    // Always show website-style menu (no expansion needed)
    render_website_menu(&egui_ctx, ctx);
    
    Ok(())
}

/// Setup custom fonts for the main menu
///
/// Tries multiple locations in order:
/// 1. Project assets/fonts/ (development)
/// 2. Executable directory (bundled app)
/// 3. System fallback (if all else fails, uses default egui font)
fn setup_custom_fonts(mut contexts: EguiContexts) {
    let ctx_result = contexts.ctx_mut();
    let Ok(ctx) = ctx_result else {
        warn!("[MAIN_MENU] Failed to get egui context for font setup");
        return;
    };

    let mut fonts = egui::FontDefinitions::default();

    // Try to find OpenSans variable font from project assets
    let possible_font_paths = [
        // Project assets (dev mode)
        "assets/fonts/OpenSans-VariableFont_wdth,wght.ttf".to_string(),
        // Relative to executable (bundled)
        "./assets/fonts/OpenSans-VariableFont_wdth,wght.ttf".to_string(),
    ];

    let mut font_loaded = false;

    for font_path in &possible_font_paths {
        if let Ok(font_data) = std::fs::read(font_path) {
            fonts.font_data.insert(
                "OpenSans".to_owned(),
                Arc::new(egui::FontData::from_owned(font_data)),
            );

            // Set OpenSans as the primary font for all text styles
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

/// Render website-style main menu with navbar and sections
fn render_website_menu(ctx: &egui::Context, ctx_menu: &mut MainMenuUIContext) {
    // Show loading screen if assets aren't loaded yet
    if !ctx_menu.loading_progress.complete {
        render_loading_screen_website(ctx, ctx_menu);
        return;
    }

    let screen_rect = ctx.content_rect();
    debug!("[MAIN_MENU] Screen rect: {:?}", screen_rect);

    // === NAVBAR ===
    render_navbar(ctx, ctx_menu);

    // === MAIN CONTENT AREA (SCROLLABLE) ===
    egui::CentralPanel::default()
        .frame(egui::Frame {
            fill: egui::Color32::from_rgba_unmultiplied(45, 45, 45, 210),
            ..Default::default()
        })
        .show(ctx, |ui| {
            ui.add_space(68.0);

            egui::ScrollArea::vertical()
                .show(ui, |ui| {
                    // Two equal columns: PLAY | QUICK PLAY
                    let available_w = ui.available_width();
                    let col_spacing = 14.0;
                    let top_col_w = ((available_w - col_spacing) / 2.0).max(0.0);

                    ui.horizontal(|ui| {
                        // === PLAY THE COMPUTER ===
                        ui.vertical(|ui| {
                            ui.set_width(top_col_w);
                            render_play_computer_section(ui, ctx_menu);
                        });

                        ui.add_space(col_spacing);

                        // === QUICK PLAY ===
                        ui.vertical(|ui| {
                            ui.set_width(top_col_w);
                            render_quick_pairing_section(ui);
                        });
                    });

                    ui.add_space(24.0);

                    // === NEWS & LEARN ROW — widths matched to a top column so
                    // the menu reads as a consistent grid. Two boxes centred
                    // horizontally with identical side margins.
                    let middle_col_spacing = col_spacing;
                    let box_width = top_col_w;
                    let side_margin =
                        ((available_w - middle_col_spacing - (box_width * 2.0)) * 0.5).max(0.0);

                    ui.horizontal(|ui| {
                        ui.add_space(side_margin); // Left margin

                        // === NEWS SECTION ===
                        ui.vertical(|ui| {
                            ui.set_width(box_width);
                            render_news_section(ui, box_width, &mut ctx_menu.news_banner);
                        });

                        ui.add_space(middle_col_spacing);

                        // === LEARN SECTION ===
                        ui.vertical(|ui| {
                            ui.set_width(box_width);
                            render_learn_section(ui, box_width, &mut ctx_menu.learn_viewport);
                        });

                        ui.add_space(side_margin); // Right margin
                    });

                    ui.add_space(30.0);

                    // === BOTTOM BOXES: TOURNAMENTS & UPDATES (equal width) ===
                    let bottom_box_height = 250.0;
                    let bottom_col_spacing = col_spacing;
                    let bottom_col_w = ((available_w - bottom_col_spacing) * 0.5).max(0.0);

                    ui.horizontal(|ui| {
                        // === TOURNAMENTS BOX ===
                        ui.vertical(|ui| {
                            ui.set_width(bottom_col_w);
                            render_tournaments_box(ui, ctx_menu);
                        });

                        ui.add_space(bottom_col_spacing);

                        // === UPDATES BOX ===
                        ui.vertical(|ui| {
                            ui.set_width(bottom_col_w);
                            render_updates_box(ui, bottom_box_height);
                        });
                    });
                });
        });

    // === AI SETUP MODAL ===
    if ctx_menu.competitive_menu.show_ai_setup {
        render_ai_setup_modal(ctx, &mut ctx_menu.competitive_menu, &mut ctx_menu.ai_config, &mut ctx_menu.core_mode, &mut ctx_menu.next_state);
    }

    // === SPECTATOR POPUP ===
    if ctx_menu.competitive_menu.show_spectator_popup {
        render_spectator_popup(ctx, &mut ctx_menu.competitive_menu);
    }

    // === CONTROLS POPUP ===
    if ctx_menu.competitive_menu.show_controls_popup {
        render_controls_popup(ctx, &mut ctx_menu.competitive_menu);
    }

    // === JOIN LOBBY POPUP ===
    if ctx_menu.competitive_menu.show_join_popup {
        render_join_lobby_popup(ctx, ctx_menu);
    }
}

/// Render website-style navbar
fn render_navbar(ctx: &egui::Context, ctx_menu: &mut MainMenuUIContext) {

    egui::TopBottomPanel::top("navbar")
        .frame(egui::Frame {
            fill: egui::Color32::from_rgba_unmultiplied(45, 45, 45, 210), // Seamless grey (matches CentralPanel)
            inner_margin: egui::Margin::symmetric(20, 15),
            outer_margin: egui::Margin::ZERO,
            ..Default::default()
        })
        .show_separator_line(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // === BRAND LOGO ===
                if let Some(texture_id) = ensure_brand_logo_texture(ui.ctx(), &mut ctx_menu.brand_logo) {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(54.0, 54.0), egui::Sense::hover());
                    ui.painter().image(
                        texture_id,
                        rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                } else {
                    ui.label(
                        egui::RichText::new("XFChess")
                            .size(24.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    );
                }

                ui.add_space(18.0);

                // === LEFT SIDE: SPECTATOR | COMMUNITY | SOURCE CODE ===
                ui.horizontal(|ui| {
                    if nav_link(ui, "Spectator") {
                        // Show spectator popup
                        info!("[MENU] Spectator clicked - opening spectator popup");
                        ctx_menu.competitive_menu.show_spectator_popup = true;
                    }
                    ui.add_space(30.0);
                    if nav_link(ui, "Community") {
                        // Open Telegram page
                        info!("[MENU] Community clicked - opening Telegram");
                        if let Err(e) = webbrowser::open("https://t.me/+IBdo42qMPqM4Y2Vk") {
                            warn!("[MENU] Failed to open Telegram: {}", e);
                        }
                    }
                    ui.add_space(30.0);
                    if nav_link(ui, "Source Code") {
                        // Open GitHub repository
                        info!("[MENU] Source Code clicked - opening GitHub");
                        if let Err(e) = webbrowser::open("https://github.com/trilltino/XFChess") {
                            warn!("[MENU] Failed to open GitHub repository: {}", e);
                        }
                    }
                    ui.add_space(30.0);
                    if nav_link(ui, "Controls") {
                        info!("[MENU] Controls clicked - opening controls popup");
                        ctx_menu.competitive_menu.show_controls_popup = true;
                    }
                });

                // === RIGHT SIDE: USERNAME DISPLAY ===
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(ref username) = ctx_menu.player_identity.username {
                        if !username.is_empty() {
                            ui.label(
                                egui::RichText::new(format!("@{}", username))
                                    .size(16.0)
                                    .color(egui::Color32::from_rgb(100, 200, 255))
                                    .strong()
                            );
                            ui.add_space(10.0);
                            ui.label(
                                egui::RichText::new("Player:")
                                    .size(14.0)
                                    .color(egui::Color32::GRAY)
                            );
                        } else {
                            ui.label(
                                egui::RichText::new("Guest")
                                    .size(14.0)
                                    .color(egui::Color32::GRAY)
                            );
                        }
                    } else {
                        ui.label(
                            egui::RichText::new("Guest")
                                .size(14.0)
                                .color(egui::Color32::GRAY)
                        );
                    }
                });

            });
        });
}

/// Render play computer section
fn render_play_computer_section(ui: &mut egui::Ui, ctx_menu: &mut MainMenuUIContext) {
    ui.vertical_centered(|ui| {
        ui.heading(
            egui::RichText::new("PLAY")
                .size(18.0)
                .color(egui::Color32::WHITE)
                .strong(),
        );
    });
    ui.add_space(15.0);

    let play_button = |text: &str| {
        egui::Button::new(
            egui::RichText::new(text)
                .size(15.0)
                .color(egui::Color32::WHITE)
                .strong(),
        )
        .fill(egui::Color32::from_rgba_unmultiplied(55, 55, 55, 200))
        .corner_radius(8.0)
        .stroke(egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30)))
    };

    let lobby_btn_resp = ui.add_sized(
        [ui.available_width(), 36.0],
        play_button("Create a Lobby"),
    );

    if lobby_btn_resp.clicked() {
        debug!("[MENU] 'Create a Lobby' clicked, setting menu state to LobbySelection");
        #[cfg(feature = "solana")]
        if let Some(ref mut lobby) = ctx_menu.solana_lobby {
            lobby.mode = crate::multiplayer::solana::lobby::LobbyMode::Create;
            lobby.status = crate::multiplayer::solana::lobby::LobbyStatus::Idle;
            lobby.wager_sol = 0.0;
        }
        ctx_menu.menu_state.set(crate::core::MenuState::LobbySelection);
    }

    ui.add_space(8.0);

    let join_btn_resp = ui.add_sized(
        [ui.available_width(), 36.0],
        play_button("Join a Lobby"),
    );

    if join_btn_resp.clicked() {
        ctx_menu.competitive_menu.show_join_popup = true;
    }

    ui.add_space(5.0);

    // Play the Computer Section
    ui.set_width(ui.available_width());
    ui.vertical(|ui| {
        if ui.add_sized(
            [ui.available_width(), 36.0],
            play_button("Play against the Computer"),
        ).clicked() {
            ctx_menu.competitive_menu.show_ai_setup = true;
        }

        ui.add_space(5.0);

        if ui.add_sized(
            [ui.available_width(), 36.0],
            play_button("Start game"),
        ).clicked() {
            info!("[MENU] Play the Computer clicked at level {}", ctx_menu.competitive_menu.ai_difficulty);
            ctx_menu.ai_config.difficulty = crate::game::ai::resource::AIDifficulty::from_u8(ctx_menu.competitive_menu.ai_difficulty);
            ctx_menu.ai_config.mode = GameMode::VsAI {
                ai_color: match ctx_menu.competitive_menu.ai_side {
                    AISide::Black => crate::rendering::pieces::PieceColor::White,
                    AISide::Random => {
                        if rand::random::<bool>() {
                            crate::rendering::pieces::PieceColor::White
                        } else {
                            crate::rendering::pieces::PieceColor::Black
                        }
                    }
                    AISide::White => crate::rendering::pieces::PieceColor::Black,
                }
            };
            *ctx_menu.core_mode = CoreGameMode::SinglePlayer;
            ctx_menu.next_state.set(GameState::InGame);
        }
    });
}

/// Render tournaments box at bottom.
fn render_tournaments_box(ui: &mut egui::Ui, ctx_menu: &mut MainMenuUIContext) {
    ui.group(|ui| {
        ui.set_width(ui.available_width());
        ui.set_height(250.0);
        ui.vertical_centered(|ui| {
            ui.heading(
                egui::RichText::new("TOURNAMENTS")
                    .size(16.0)
                    .color(egui::Color32::WHITE)
                    .strong(),
            );
        });
        ui.add_space(10.0);
        ui.vertical(|ui| {
            let mut tournaments_found = false;
            if let Some(vps_state) = ctx_menu.p2p_vps_state.as_ref() {
                for listing in &vps_state.cached_games {
                    if listing.game_type == "tournament" {
                        tournaments_found = true;
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new(&listing.display_name)
                                        .size(12.0)
                                        .color(egui::Color32::WHITE)
                                        .strong(),
                                );
                                let prize = if listing.stake_amount > 0.0 {
                                    format!("{:.3} SOL", listing.stake_amount)
                                } else {
                                    "Free".to_string()
                                };
                                ui.label(
                                    egui::RichText::new(prize)
                                        .size(10.0)
                                        .color(egui::Color32::from_rgb(150, 200, 150)),
                                );
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.add_sized(
                                    [60.0, 24.0],
                                    egui::Button::new(
                                        egui::RichText::new("Join")
                                            .size(11.0)
                                            .color(egui::Color32::WHITE)
                                            .strong(),
                                    )
                                    .fill(egui::Color32::from_rgb(100, 200, 100))
                                    .corner_radius(6.0)
                                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30))),
                                ).clicked() {
                                    ctx_menu.next_state.set(GameState::InGame);
                                }
                            });
                        });
                        ui.add_space(8.0);
                    }
                }
            }

            if !tournaments_found {
                ui.label(
                    egui::RichText::new("No active tournaments")
                        .size(11.0)
                        .color(egui::Color32::from_rgb(120, 120, 120))
                        .italics(),
                );
            }
        });
    });
}

/// Render updates box at bottom.
fn render_updates_box(ui: &mut egui::Ui, box_height: f32) {
    ui.group(|ui| {
        ui.set_width(ui.available_width());
        ui.set_height(box_height);
        ui.vertical_centered(|ui| {
            ui.heading(
                egui::RichText::new("UPDATES")
                    .size(16.0)
                    .color(egui::Color32::WHITE)
                    .strong(),
            );
        });
        ui.add_space(10.0);
        ui.vertical(|ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(30.0);
                ui.label(
                    egui::RichText::new("XFChess Released stay tuned for updates!")
                        .size(14.0)
                        .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 200))
                        .italics(),
                );
            });
        });
    });
}

/// Render middle quick pairing section
fn render_quick_pairing_section(ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.heading(
            egui::RichText::new("QUICK PLAY")
                .size(18.0)
                .color(egui::Color32::WHITE)
                .strong(),
        );
    });
    ui.add_space(15.0);

    // Online wager tiers (locked until wallet connected)
    let wagers = [("£2 Wager", 0.05), ("£5 Wager", 0.12), ("£10 Wager", 0.25)];
    for (name, _stake) in wagers {
        let btn_text = format!("{} (Locked)", name);
        let resp = ui.add_sized(
            [ui.available_width(), 36.0],
            egui::Button::new(
                egui::RichText::new(btn_text)
                    .size(13.0)
                    .color(egui::Color32::from_rgb(120, 120, 120))
                    .strong(),
            )
            .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 150))
            .corner_radius(8.0)
            .stroke(egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30))),
        ).on_hover_text("Connect wallet to wager");

        if resp.clicked() {
            info!("[MENU] Wager {} clicked — wallet not connected, opening sign-in", name);
            let _ = webbrowser::open("http://localhost:7454/auth/login");
        }
        ui.add_space(5.0);
    }
}

/// Render news section with the provided screenshot banner.
fn render_news_section(ui: &mut egui::Ui, box_width: f32, news_banner: &mut NewsBannerState) {
    ui.vertical_centered(|ui| {
        ui.heading(
            egui::RichText::new("NEWS")
                .size(18.0)
                .color(egui::Color32::WHITE)
                .strong(),
        );
    });
    ui.add_space(10.0);

    // The screenshot is a wide banner, so we keep the section borderless and
    // let the image define the visual weight of the card.
    ui.vertical_centered(|ui| {
        if let Some(texture_id) = ensure_news_banner_texture(ui.ctx(), news_banner) {
            let banner_w = box_width * 0.98;
            let banner_h = banner_w * 0.50;
            let (rect, response) = ui.allocate_exact_size(egui::vec2(banner_w, banner_h), egui::Sense::click());

            ui.painter().image(
                texture_id,
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );

            if response.clicked() {
                info!("[MENU] News banner clicked - opening release page");
                if let Err(e) = webbrowser::open("http://localhost:5173/news/release") {
                    warn!("[MENU] Failed to open release notes: {}", e);
                }
            }
        } else {
            let fallback_size = egui::vec2(box_width * 0.98, box_width * 0.50);
            let (rect, response) = ui.allocate_exact_size(fallback_size, egui::Sense::click());
            ui.painter().rect_filled(rect, 8.0, egui::Color32::from_rgba_unmultiplied(55, 55, 55, 200));
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "XFChess released",
                egui::FontId::proportional(16.0),
                egui::Color32::WHITE,
            );
            if response.clicked() {
                info!("[MENU] News banner fallback clicked - opening release page");
                if let Err(e) = webbrowser::open("http://localhost:5173/news/release") {
                    warn!("[MENU] Failed to open release notes: {}", e);
                }
            }
        }
    });
}

/// Render learn section (formerly featured game) - square shape.
/// The main square is an empty egui area that acts as a borderless window for
/// the mini 3D showcase camera spawned by `XfAnimatePlugin`. The allocated
/// rect (in physical pixels) is written to `LearnViewportRect` so the camera
/// viewport tracks egui's layout.
fn render_learn_section(
    ui: &mut egui::Ui,
    box_width: f32,
    learn_viewport: &mut crate::xf_animate::LearnViewportRect,
) {
    ui.vertical_centered(|ui| {
        ui.heading(
            egui::RichText::new("LEARN")
                .size(18.0)
                .color(egui::Color32::WHITE)
                .strong(),
        );
    });
    ui.add_space(6.0);

    ui.label(
        egui::RichText::new("Immortal Zugzwang Game")
            .size(14.0)
            .color(egui::Color32::WHITE)
            .strong(),
    );
    ui.add_space(2.0);
    ui.label(
        egui::RichText::new("Sämisch vs Nimzowitsch, 1923")
            .size(11.0)
            .color(egui::Color32::from_rgb(150, 150, 150))
            .italics(),
    );
    ui.add_space(6.0);

    // Reserve a perfect square for the mini-board viewport. Guard against
    // pathological layouts (negative/zero available width) — egui panics on
    // negative dimensions, and wgpu rejects scissor rects beyond the render
    // target, so we simply hide the camera in those cases.
    let available = ui.available_width().max(0.0);
    let side = box_width.min(available).max(0.0) * 0.92;
    const MIN_SIDE: f32 = 48.0;

    if side < MIN_SIDE {
        learn_viewport.rect_px = None;
    } else {
        ui.vertical_centered(|ui| {
            let (rect, _response) = ui.allocate_exact_size(
                egui::vec2(side, side),
                egui::Sense::hover(),
            );
            let ppp = ui.ctx().pixels_per_point();
            learn_viewport.rect_px =
                Some(crate::xf_animate::viewport::egui_rect_to_pixels(rect, ppp));
        });
    }

}

/// Render middle lobby section with live VPS listings and type filter.
fn render_lobby_section(ui: &mut egui::Ui, ctx_menu: &mut MainMenuUIContext) {
    ui.group(|ui| {
        ui.set_width(ui.available_width());
        ui.set_height(250.0);
        ui.vertical_centered(|ui| {
            ui.heading(
                egui::RichText::new("LOBBY")
                    .size(16.0)
                    .color(egui::Color32::WHITE)
                    .strong(),
            );
        });
        ui.add_space(10.0);
        ui.vertical(|ui| {
            let mut tournaments_found = false;
            if let Some(vps_state) = ctx_menu.p2p_vps_state.as_ref() {
                for listing in &vps_state.cached_games {
                    if listing.game_type == "tournament" {
                        tournaments_found = true;
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new(&listing.display_name)
                                        .size(12.0)
                                        .color(egui::Color32::WHITE)
                                        .strong(),
                                );
                                let prize = if listing.stake_amount > 0.0 {
                                    format!("{:.3} SOL", listing.stake_amount)
                                } else {
                                    "Free".to_string()
                                };
                                ui.label(
                                    egui::RichText::new(prize)
                                        .size(10.0)
                                        .color(egui::Color32::from_rgb(150, 200, 150)),
                                );
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.add_sized(
                                    [60.0, 24.0],
                                    egui::Button::new(
                                        egui::RichText::new("Join")
                                            .size(11.0)
                                            .color(egui::Color32::WHITE)
                                            .strong(),
                                    )
                                    .fill(egui::Color32::from_rgb(100, 200, 100))
                                    .corner_radius(6.0)
                                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30))),
                                ).clicked() {
                                    ctx_menu.next_state.set(GameState::InGame);
                                }
                            });
                        });
                        ui.add_space(8.0);
                    }
                }
            }

            if !tournaments_found {
                ui.label(
                    egui::RichText::new("No active tournaments")
                        .size(11.0)
                        .color(egui::Color32::from_rgb(120, 120, 120))
                        .italics(),
                );
            }
        });
    });
}

/// Navbar link helper — plain clickable text, no box.
fn nav_link(ui: &mut egui::Ui, text: &str) -> bool {
    let response = ui.add(
        egui::Label::new(
            egui::RichText::new(text)
                .size(14.0)
                .color(egui::Color32::from_rgb(200, 200, 200)),
        )
        .sense(egui::Sense::click()),
    );

    if response.hovered() {
        ui.painter().text(
            response.rect.left_bottom(),
            egui::Align2::LEFT_BOTTOM,
            text,
            egui::FontId::proportional(14.0),
            egui::Color32::WHITE,
        );
    }

    response.clicked()
}

/// Navbar button helper
fn nav_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let response = ui.add(
        egui::Button::new(
            egui::RichText::new(text)
                .size(14.0)
                .color(egui::Color32::WHITE),
        )
        .fill(egui::Color32::from_rgba_unmultiplied(60, 60, 60, 200))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 100))),
    );
    
    if response.hovered() {
        ui.painter().rect(
            response.rect,
            0.0,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30),
            egui::Stroke::NONE,
            egui::epaint::StrokeKind::Middle,
        );
    }

    response
}

/// Grey bezel border color for popups
const BEZEL_GREY: egui::Color32 = egui::Color32::from_rgb(100, 100, 100);

/// Render AI setup modal with strength and side selection
fn render_ai_setup_modal(
    ctx: &egui::Context,
    competitive: &mut CompetitiveMenuState,
    ai_config: &mut crate::game::ai::resource::ChessAIResource,
    core_mode: &mut CoreGameMode,
    next_state: &mut NextState<GameState>,
) {
    let accent_color = egui::Color32::from_rgb(173, 92, 47); // #ad5c2f

    egui::Window::new("Game Setup")
        .collapsible(false)
        .resizable(false)
        .fixed_size(egui::Vec2::new(380.0, 400.0))
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .frame(egui::Frame {
            fill: egui::Color32::from_rgba_unmultiplied(30, 30, 30, 240),
            corner_radius: egui::Rounding::same(4),
            stroke: egui::Stroke::new(2.0, BEZEL_GREY),
            inner_margin: egui::Margin::same(16),
            ..Default::default()
        })
        .show(ctx, |ui| {
            // Close button only (window title already shows "Game Setup")
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("X").clicked() {
                        competitive.show_ai_setup = false;
                    }
                });
            });

            ui.add_space(12.0);

            // Strength section
            ui.label(
                egui::RichText::new("Strength")
                    .size(14.0)
                    .color(egui::Color32::WHITE)
                    .strong(),
            );
            ui.add_space(6.0);

            // Strength grid (1-8) - compact
            let elos = [0, 400, 700, 1000, 1300, 1600, 1900, 2200, 2500];
            ui.horizontal(|ui| {
                for lvl in 1..=8 {
                    let response = ui.add(
                        egui::Button::new(
                            egui::RichText::new(format!("{}", lvl))
                                .size(14.0)
                                .color(if competitive.ai_difficulty == lvl {
                                    egui::Color32::WHITE
                                } else {
                                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 160)
                                })
                        )
                        .min_size(egui::Vec2::new(32.0, 32.0))
                        .fill(if competitive.ai_difficulty == lvl {
                            accent_color
                        } else {
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 5)
                        })
                        .corner_radius(4.0)
                        .stroke(egui::Stroke::new(
                            1.0,
                            if competitive.ai_difficulty == lvl {
                                accent_color
                            } else {
                                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 10)
                            }
                        ))
                    );

                    if response.clicked() {
                        competitive.ai_difficulty = lvl;
                    }
                    ui.add_space(4.0);
                }
            });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("{} ELO", elos[competitive.ai_difficulty as usize]))
                        .size(11.0)
                        .color(egui::Color32::from_rgb(150, 150, 150)),
                );
            });

            ui.add_space(16.0);

            // Side selection (buttons are self-explanatory)
            ui.add_space(10.0);

            // Side selection — clicking a side immediately starts the game with
            // that side as the player's choice (no separate Play button).
            ui.horizontal(|ui| {
                let mut picked: Option<AISide> = None;
                for (label, side) in [
                    ("Black", AISide::Black),
                    ("Random", AISide::Random),
                    ("White", AISide::White),
                ] {
                    let selected = competitive.ai_side == side;
                    let btn = egui::Button::new(egui::RichText::new(label).size(14.0))
                        .min_size(egui::Vec2::new(70.0, 40.0))
                        .corner_radius(4.0)
                        .fill(if selected {
                            accent_color
                        } else {
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 5)
                        })
                        .stroke(egui::Stroke::new(
                            1.0,
                            if selected {
                                accent_color
                            } else {
                                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 10)
                            },
                        ));
                    if ui.add(btn).clicked() {
                        competitive.ai_side = side;
                        picked = Some(side);
                    }
                    ui.add_space(8.0);
                }

                if let Some(side) = picked {
                    info!("[MENU] AI setup modal - side selected: {:?} — starting game", side);
                    ai_config.difficulty = crate::game::ai::resource::AIDifficulty::from_u8(competitive.ai_difficulty);
                    ai_config.mode = GameMode::VsAI {
                        ai_color: match side {
                            AISide::Black => crate::rendering::pieces::PieceColor::White,
                            AISide::Random => {
                                if rand::random::<bool>() {
                                    crate::rendering::pieces::PieceColor::White
                                } else {
                                    crate::rendering::pieces::PieceColor::Black
                                }
                            }
                            AISide::White => crate::rendering::pieces::PieceColor::Black,
                        },
                    };
                    *core_mode = CoreGameMode::SinglePlayer;
                    next_state.set(GameState::InGame);
                    competitive.show_ai_setup = false;
                }
            });
        });
}

/// Render controls / keybindings popup reached from the navbar.
fn render_controls_popup(ctx: &egui::Context, competitive: &mut CompetitiveMenuState) {
    egui::Window::new("Controls")
        .collapsible(false)
        .resizable(false)
        .fixed_size(egui::Vec2::new(420.0, 360.0))
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .title_bar(false)
        .frame(egui::Frame {
            fill: egui::Color32::from_rgba_unmultiplied(30, 30, 30, 240),
            corner_radius: egui::Rounding::same(4),
            stroke: egui::Stroke::new(2.0, BEZEL_GREY),
            inner_margin: egui::Margin::same(16),
            ..Default::default()
        })
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Controls")
                        .size(18.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("X").clicked() {
                        competitive.show_controls_popup = false;
                    }
                });
            });

            ui.add_space(12.0);

            ui.label(
                egui::RichText::new("Controls")
                    .size(14.0)
                    .color(egui::Color32::WHITE)
                    .strong(),
            );
            ui.add_space(6.0);

            let rows: [(&str, &str); 8] = [
                ("Left Click", "Select piece / confirm move"),
                ("Right Click", "Deselect / cancel"),
                ("Mouse Wheel", "Zoom camera"),
                ("Middle Drag", "Orbit camera"),
                ("Esc", "Pause / back to menu"),
                ("R", "Reset camera view"),
                ("F", "Flip board"),
                ("U", "Undo last move (local only)"),
            ];

            for (key, desc) in rows {
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [130.0, 20.0],
                        egui::Label::new(
                            egui::RichText::new(key)
                                .size(13.0)
                                .color(egui::Color32::from_rgb(220, 180, 120))
                                .strong(),
                        ),
                    );
                    ui.label(
                        egui::RichText::new(desc)
                            .size(13.0)
                            .color(egui::Color32::from_rgb(210, 210, 210)),
                    );
                });
                ui.add_space(4.0);
            }
        });
}

/// Render spectator popup to view all games
fn render_spectator_popup(ctx: &egui::Context, competitive: &mut CompetitiveMenuState) {
    let accent_color = egui::Color32::from_rgb(173, 92, 47); // #ad5c2f

    egui::Window::new("Spectator Mode")
        .collapsible(false)
        .resizable(false)
        .fixed_size(egui::Vec2::new(500.0, 320.0))
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .title_bar(false)
        .frame(egui::Frame {
            fill: egui::Color32::from_rgba_unmultiplied(30, 30, 30, 240),
            corner_radius: egui::Rounding::same(4),
            stroke: egui::Stroke::new(2.0, BEZEL_GREY),
            inner_margin: egui::Margin::same(16),
            ..Default::default()
        })
        .show(ctx, |ui| {
            // Header with X close button
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Spectator Mode")
                        .size(18.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("X").clicked() {
                        competitive.show_spectator_popup = false;
                    }
                });
            });

            ui.add_space(12.0);

            ui.label(
                egui::RichText::new("No games available to spectate at the moment.")
                    .size(14.0)
                    .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180)),
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("Check back later for live games.")
                    .size(12.0)
                    .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 120))
                    .italics(),
            );

            ui.add_space(20.0);

            if ui.add_sized(
                [ui.available_width(), 36.0],
                egui::Button::new(
                    egui::RichText::new("Close")
                        .size(14.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                )
                .fill(accent_color)
                .corner_radius(4.0)
            ).clicked() {
                competitive.show_spectator_popup = false;
            }
        });
}

/// Loading screen for website menu
fn render_loading_screen_website(ctx: &egui::Context, ctx_menu: &mut MainMenuUIContext) {
    let screen_rect = ctx.input(|i| i.content_rect());
    
    egui::CentralPanel::default()
        .frame(egui::Frame {
            fill: egui::Color32::from_rgb(0, 0, 0),
            ..Default::default()
        })
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(screen_rect.height() * 0.4);
                
                if ctx_menu.loading_progress.failed {
                    ui.heading(
                        egui::RichText::new("Asset Loading Failed")
                            .size(24.0)
                            .color(egui::Color32::from_rgb(220, 50, 50)),
                    );
                    
                    if let Some(ref error_msg) = ctx_menu.loading_progress.error_message {
                        ui.label(
                            egui::RichText::new(error_msg)
                                .size(14.0)
                                .color(egui::Color32::from_rgb(220, 150, 150)),
                        );
                    }
                    
                    if ui.button("Continue Anyway").clicked() {
                        ctx_menu.loading_progress.complete = true;
                        ctx_menu.game_assets.loaded = true;
                    }
                } else {
                    ui.heading(
                        egui::RichText::new("Loading...")
                            .size(24.0)
                            .color(egui::Color32::WHITE),
                    );
                    
                    let progress_bar = egui::ProgressBar::new(ctx_menu.loading_progress.progress)
                        .desired_width(300.0)
                        .show_percentage()
                        .animate(true);
                    
                    ui.add(progress_bar);
                }
            });
        });
}

fn render_loading_screen(ui: &mut egui::Ui, ctx: &mut MainMenuUIContext) {
    ui.vertical_centered(|ui| {
        Layout::small_space(ui);

        // Check if loading failed
        if ctx.loading_progress.failed {
            // Error state
            ui.heading(
                egui::RichText::new("Asset Loading Failed")
                    .size(20.0)
                    .color(egui::Color32::from_rgb(220, 50, 50)),
            );

            Layout::small_space(ui);

            // Error message
            if let Some(ref error_msg) = ctx.loading_progress.error_message {
                ui.label(
                    egui::RichText::new(error_msg)
                        .size(12.0)
                        .color(egui::Color32::from_rgb(220, 150, 150)),
                );
            } else {
                ui.label(
                    egui::RichText::new("Failed to load required assets")
                        .size(12.0)
                        .color(egui::Color32::from_rgb(220, 150, 150)),
                );
            }

            Layout::small_space(ui);

            // Option to continue anyway
            if ui.button("Continue Anyway (May cause issues)").clicked() {
                warn!("[MAIN_MENU] User chose to continue despite asset loading failure");
                ctx.loading_progress.complete = true;
                ctx.loading_progress.progress = 1.0;
                ctx.game_assets.loaded = true;
                info!("[MAIN_MENU] Asset loading marked as complete despite failure");
            }
        } else {
            // Loading state
            ui.heading(
                egui::RichText::new("Loading...")
                    .size(20.0)
                    .color(egui::Color32::from_rgb(220, 220, 220)),
            );

            Layout::small_space(ui);

            // Progress bar
            let progress_bar = egui::ProgressBar::new(ctx.loading_progress.progress)
                .desired_width(300.0)
                .show_percentage()
                .animate(true);

            ui.add(progress_bar);

            Layout::small_space(ui);

            // Status text
            ui.label(
                egui::RichText::new("Loading assets...")
                    .size(12.0)
                    .color(egui::Color32::from_rgb(180, 180, 180)),
            );
        }

        Layout::small_space(ui);
    });
}

// === SUB-MENUS ===

/// Helper to create a styled bezel button with border and hover effect
fn bezel_button(ui: &mut egui::Ui, text: &str, color: egui::Color32) -> bool {
    let frame = egui::Frame {
        fill: egui::Color32::from_rgba_unmultiplied(0, 0, 0, 100),
        stroke: egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 80)),
        inner_margin: egui::Margin::symmetric(12, 8),
        ..Default::default()
    };

    let response = frame
        .show(ui, |ui| {
            // Use horizontal layout instead of vertical_centered
            ui.horizontal_centered(|ui| {
                ui.label(
                    egui::RichText::new(text)
                        .size(16.0)
                        .color(color)
                        .strong(),
                );
            })
        })
        .response;

    // Add click interaction to the frame's response area
    let interact_response = ui.interact(response.rect, response.id, egui::Sense::click());

    // Hover effect
    if interact_response.hovered() {
        ui.painter().rect(
            response.rect,
            0.0,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 20),
            egui::Stroke::new(2.0, color),
            egui::epaint::StrokeKind::Middle,
        );
    }

    interact_response.clicked()
}

/// Compact bezel button for horizontal layouts (smaller padding)
fn bezel_button_compact(ui: &mut egui::Ui, text: &str, color: egui::Color32) -> bool {
    let frame = egui::Frame {
        fill: egui::Color32::from_rgba_unmultiplied(0, 0, 0, 100),
        stroke: egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 80)),
        inner_margin: egui::Margin::symmetric(8, 4),
        ..Default::default()
    };

    let response = frame
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(
                    egui::RichText::new(text)
                        .size(14.0)
                        .color(color)
                        .strong(),
                );
            })
        })
        .response;

    let interact_response = ui.interact(response.rect, response.id, egui::Sense::click());

    if interact_response.hovered() {
        ui.painter().rect(
            response.rect,
            0.0,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 20),
            egui::Stroke::new(2.0, color),
            egui::epaint::StrokeKind::Middle,
        );
    }

    interact_response.clicked()
}


fn ui_main(ui: &mut egui::Ui, ctx: &mut MainMenuUIContext) {
    ui.vertical_centered(|ui| {
        Layout::section_space(ui);

        // Styled bezel buttons
        if bezel_button(ui, "PLAY", egui::Color32::WHITE) {
            ctx.menu_state.set(crate::core::MenuState::ModeSelect);
        }

        Layout::item_space(ui);

        if bezel_button(ui, "EXIT", egui::Color32::from_rgb(200, 100, 100)) {
            std::process::exit(0);
        }
    });

}

fn ui_mode_select(ui: &mut egui::Ui, ctx: &mut MainMenuUIContext) {
    ui.vertical_centered(|ui| {
        Layout::section_space(ui);

        // Back button as text
        if ui
            .add(
                egui::Label::new(
                    egui::RichText::new("← BACK")
                        .size(16.0)
                        .color(egui::Color32::from_rgb(150, 150, 150)),
                )
                .sense(egui::Sense::click()),
            )
            .clicked()
        {
            ctx.menu_state.set(crate::core::MenuState::Main);
        }

        Layout::section_space(ui);

        ui.label(
            egui::RichText::new("SELECT GAME MODE")
                .size(24.0)
                .color(egui::Color32::WHITE)
                .strong(),
        );

        Layout::item_space(ui);

        // --- LOCAL PLAY ---
        ui.label(
            egui::RichText::new("LOCAL PLAY")
                .size(20.0)
                .color(egui::Color32::from_rgb(200, 200, 200))
                .strong(),
        );
        Layout::small_space(ui);

        if bezel_button(ui, "♟ VS Local Friend (PvP)", egui::Color32::WHITE) {
            ctx.ai_config.mode = GameMode::Multiplayer;
            ctx.next_state.set(GameState::InGame);
            info!("[MAIN_MENU] Starting Local PvP game");
        }

        Layout::small_space(ui);

        let ai_color = crate::rendering::pieces::PieceColor::Black;

        ui.horizontal(|ui| {
            ui.label("Difficulty:");
            ui.add_space(8.0);
            
            let mut current_idx = ctx.ai_config.difficulty.to_u8();
            if ui.add(egui::Slider::new(&mut current_idx, 1..=8).show_value(true)).changed() {
                ctx.ai_config.difficulty = crate::game::ai::resource::AIDifficulty::from_u8(current_idx);
            }
        });

        ui.label(
            egui::RichText::new(ctx.ai_config.difficulty.description())
                .size(15.0)
                .color(egui::Color32::from_rgb(100, 200, 255))
                .strong()
        );

        Layout::small_space(ui);

        if bezel_button(ui, "🚀 PLAY VS COMPUTER", egui::Color32::from_rgb(100, 200, 255)) {
            ctx.ai_config.mode = GameMode::VsAI { ai_color };
            *ctx.core_mode = CoreGameMode::SinglePlayer;
            ctx.next_state.set(GameState::InGame);
            info!("[MAIN_MENU] Starting VS Computer ({})", ctx.ai_config.difficulty.description());
        }

        Layout::item_space(ui);
        ui.separator();
        Layout::item_space(ui);

        // --- SOLANA P2P ---
        #[cfg(feature = "solana")]
        ui.label(
            egui::RichText::new("SOLANA P2P")
                .size(20.0)
                .color(egui::Color32::from_rgb(255, 150, 100))
                .strong(),
        );
        #[cfg(feature = "solana")]
        Layout::small_space(ui);

        #[cfg(feature = "solana")]
        if bezel_button(ui, "🏆 Tournaments", egui::Color32::from_rgb(255, 150, 100)) {
            ctx.menu_state.set(crate::core::MenuState::Tournaments);
            info!("[MAIN_MENU] Entering Tournament Browser");
        }

        #[cfg(feature = "solana")]
        Layout::small_space(ui);

        #[cfg(feature = "solana")]
        if bezel_button(ui, "Solana Lobby", egui::Color32::from_rgb(255, 150, 100)) {
            ctx.menu_state.set(crate::core::MenuState::SolanaLobby);
            info!("[MAIN_MENU] Entering Solana Lobby");
        }

        #[cfg(feature = "solana")]
        Layout::item_space(ui);
        #[cfg(feature = "solana")]
        ui.separator();
        #[cfg(feature = "solana")]
        Layout::item_space(ui);

        // --- P2P CHESS (IROH/BRAID) ---
        ui.label(
            egui::RichText::new("GLOBAL P2P")
                .size(20.0)
                .color(egui::Color32::from_rgb(100, 200, 255))
                .strong(),
        );
        Layout::small_space(ui);

        // VPS Mode Indicator
        let vps_status = if false {
            "VPS Mode: ON"
        } else {
            "VPS Mode: OFF"
        };
        let vps_color = egui::Color32::from_rgb(150, 150, 150); // Default to OFF color
        ui.label(
            egui::RichText::new(vps_status)
                .size(12.0)
                .color(vps_color),
        );
        Layout::small_space(ui);

        // Display Node ID
        ui.label(
            egui::RichText::new("Your Node ID:")
                .size(14.0)
                .color(egui::Color32::from_rgb(150, 150, 150)),
        );

        let node_id_display = "Initializing...".to_string();

        ui.label(
            egui::RichText::new(node_id_display)
                .size(16.0)
                .color(egui::Color32::from_rgb(100, 200, 255))
                .monospace(),
        );

        Layout::item_space(ui);
        ui.separator();
        Layout::item_space(ui);
    });
}

// --- P2P LOBBY UI ---
// Disabled - lightyear dependencies removed

// --- BRAID LOBBY UI ---
// Disabled - lightyear dependencies removed

#[cfg(feature = "solana")]
fn ui_solana_lobby(ui: &mut egui::Ui, ctx: &mut MainMenuUIContext) {
    let Some(ref mut lobby) = ctx.solana_lobby else {
        ui.label("Solana lobby not available.");
        return;
    };

    ui.vertical_centered(|ui| {
        Layout::section_space(ui);

        if ui.button("⬅ Back").clicked() {
            ctx.menu_state.set(crate::core::MenuState::ModeSelect);
            lobby.status = LobbyStatus::Idle;
        }

        Layout::section_space(ui);

        ui.label(
            egui::RichText::new("SOLANA WAGER LOBBY")
                .size(24.0)
                .color(egui::Color32::from_rgb(255, 150, 100))
                .strong(),
        );

        Layout::item_space(ui);

        // Wallet / balance header
        let balance = lobby.cached_balance;
        let wallet_ready = lobby.cached_keypair_bytes.is_some();
        if wallet_ready {
            ui.label(
                egui::RichText::new(format!("Wallet balance: {:.4} SOL", balance))
                    .color(egui::Color32::GOLD),
            );
        } else {
            ui.colored_label(egui::Color32::YELLOW, "Wallet not connected");
            if ui.button("🔗 Connect Wallet").clicked() {
                crate::multiplayer::solana::tauri_signer::open_wallet_browser();
            }
        }

        // Node ID display disabled - lightyear dependencies removed

        Layout::item_space(ui);

        // Only show the create/join form while not in a post-transaction state.
        let in_post_state = matches!(
            lobby.status,
            LobbyStatus::WaitingForOpponent { .. }
                | LobbyStatus::OpponentJoined { .. }
                | LobbyStatus::Success(_)
        );

        if !in_post_state {
            // Tab switcher
            ui.horizontal(|ui| {
                if ui.selectable_label(lobby.mode == LobbyMode::Create, "Create Game").clicked() {
                    lobby.mode = LobbyMode::Create;
                    lobby.status = LobbyStatus::Idle;
                }
                if ui.selectable_label(lobby.mode == LobbyMode::Join, "Join Game").clicked() {
                    lobby.mode = LobbyMode::Join;
                    lobby.status = LobbyStatus::Idle;
                }
            });

            ui.separator();
            Layout::item_space(ui);

            match lobby.mode {
                LobbyMode::Create => render_create_tab(ui, lobby, &mut ctx.compliance),
                LobbyMode::Join => render_join_tab(ui, lobby),
            }
        }

        Layout::item_space(ui);

        // Auto-transition: Success + Create mode → WaitingForOpponent + start poll.
        if let LobbyStatus::Success(game_id) = lobby.status {
            if lobby.mode == LobbyMode::Create && lobby.opponent_poll_rx.is_none() {
                let (tx, rx) = tokio::sync::oneshot::channel();
                spawn_poll_opponent_joined(lobby.cached_rpc_url.clone(), game_id, tx);
                lobby.opponent_poll_rx = Some(rx);
                lobby.status = LobbyStatus::WaitingForOpponent { game_id };
            }
        }

        // Post-action status UI (uses ctx for firing events).
        let status_snap = lobby.status.clone();
        let wager_lamports = lobby.wager_lamports();

        match status_snap {
            LobbyStatus::Idle => {}

            LobbyStatus::Pending => {
                ui.spinner();
                ui.label(
                    egui::RichText::new("⏳ Submitting transaction...")
                        .color(egui::Color32::from_rgb(200, 200, 50)),
                );
            }

            LobbyStatus::WaitingForOpponent { game_id } => {
                ui.spinner();
                ui.label(
                    egui::RichText::new(format!("⏳ Game #{} — waiting for opponent to join on-chain...", game_id))
                        .color(egui::Color32::from_rgb(255, 200, 80)),
                );
                Layout::small_space(ui);
                ui.label(
                    egui::RichText::new("Share your Node ID with your opponent:")
                        .size(12.0)
                        .color(egui::Color32::LIGHT_GRAY),
                );
                // Temporarily disabled to remove lightyear dependencies
                /*
                if let Some(node_id) = &ctx.network_state.node_id {
                    let full = bs58::encode(node_id.as_bytes()).into_string();
                    let short = format!("{:.16}...", full);
                    ui.label(
                        egui::RichText::new(&short)
                            .size(13.0)
                            .color(egui::Color32::from_rgb(100, 200, 255))
                            .monospace(),
                    );
                    if ui.small_button("📋 Copy Node ID").clicked() {
                        ui.output_mut(|o| {
                            o.commands
                                .push(egui::OutputCommand::CopyText(full.clone()));
                        });
                        info!("[SOLANA_LOBBY] Node ID copied while waiting: {}", full);
                    }
                }
                */
                Layout::small_space(ui);
                if ui.small_button("✖ Cancel").clicked() {
                    lobby.status = LobbyStatus::Idle;
                    lobby.opponent_poll_rx = None;
                }
            }

            LobbyStatus::OpponentJoined { game_id } => {
                ui.label(
                    egui::RichText::new("✅ Opponent joined on-chain!")
                        .color(egui::Color32::from_rgb(100, 255, 100))
                        .strong(),
                );
                Layout::small_space(ui);
                ui.label(
                    egui::RichText::new(
                        "Click 'Host Game' — opponent will enter your Node ID to connect.",
                    )
                    .size(12.0)
                    .color(egui::Color32::LIGHT_GRAY),
                );
                Layout::small_space(ui);
                if ui.button("🎮 Host Game").clicked() {
                    ctx.ai_config.mode = GameMode::Multiplayer;
                    *ctx.core_mode = CoreGameMode::BraidMultiplayer;
                    if let Some(ref mut sync) = ctx.solana_sync {
                        sync.game_id = Some(game_id);
                        sync.wager_amount = wager_lamports;
                    }
                    if let Some(ref mut comp) = ctx.competitive {
                        comp.game_id = Some(game_id);
                        comp.wager_lamports = wager_lamports;
                        comp.active = true;
                    }
                    info!("[SOLANA_LOBBY] Hosting P2P for on-chain game #{}", game_id);
                }
            }

            // Join success — show Node ID entry so joiner can connect to host.
            LobbyStatus::Success(game_id) => {
                ui.label(
                    egui::RichText::new(format!("✅ Joined game #{}!", game_id))
                        .color(egui::Color32::GREEN)
                        .strong(),
                );
                Layout::small_space(ui);
                ui.label(
                    egui::RichText::new("Enter host's Node ID to start playing:")
                        .size(13.0)
                        .color(egui::Color32::LIGHT_GRAY),
                );

                let response = ui.text_edit_singleline(&mut String::new());
                if response.changed() {}
                Layout::small_space(ui);
                if ui.button("🔗 Connect to Host").clicked() {
                    ctx.ai_config.mode = GameMode::Multiplayer;
                    if let Some(ref mut sync) = ctx.solana_sync {
                        sync.game_id = Some(game_id);
                        sync.wager_amount = wager_lamports;
                    }
                    if let Some(ref mut comp) = ctx.competitive {
                        comp.game_id = Some(game_id);
                        comp.wager_lamports = wager_lamports;
                        comp.active = true;
                    }
                    info!("[SOLANA_LOBBY] Connecting to host for game #{}", game_id);
                }
            }

            LobbyStatus::Fetched { .. } => {}

            LobbyStatus::Error(msg) => {
                ui.colored_label(egui::Color32::RED, format!("❌ {}", msg));
                if ui.small_button("↩ Try Again").clicked() {
                    lobby.status = LobbyStatus::Idle;
                }
            }
        }
    });
}

#[cfg(feature = "solana")]
fn render_create_tab(
    ui: &mut egui::Ui,
    lobby: &mut crate::multiplayer::solana::lobby::SolanaLobbyState,
    compliance: &mut crate::ui::compliance_modal::ComplianceState,
) {
    let balance = lobby.cached_balance;
    let wallet_connected = lobby.cached_keypair_bytes.is_some();
    let max_wager = if wallet_connected {
        ((balance - 0.002) as f32).max(0.0)
    } else {
        0.0
    };

    // If wallet is not connected force wager to 0 (free game only).
    if !wallet_connected {
        lobby.wager_sol = 0.0;
    }

    ui.label(egui::RichText::new("Wager amount (SOL)").size(14.0));

    if wallet_connected {
        ui.add(
            egui::Slider::new(&mut lobby.wager_sol, 0.0..=max_wager.max(0.001))
                .step_by(0.001)
                .fixed_decimals(3),
        );
        // Clamp after slider interaction
        lobby.wager_sol = lobby.wager_sol.clamp(0.0, max_wager);
    } else {
        // Greyed-out disabled slider at 0
        let mut zero: f32 = 0.0;
        ui.add_enabled(
            false,
            egui::Slider::new(&mut zero, 0.0..=1.0).fixed_decimals(3),
        );
        ui.label(
            egui::RichText::new("(Connect wallet to add a wager)")
                .size(11.0)
                .color(egui::Color32::from_rgb(160, 130, 80))
                .italics(),
        );
    }

    let label_text = if lobby.wager_sol == 0.0 {
        "Free casual game — no SOL at stake".to_string()
    } else {
        format!("Escrow: {:.4} SOL  |  Pot: {:.4} SOL",
            lobby.wager_sol, lobby.wager_sol * 2.0)
    };
    ui.label(
        egui::RichText::new(label_text)
            .color(egui::Color32::LIGHT_GRAY)
            .size(12.0),
    );

    Layout::small_space(ui);

    let is_free_game = lobby.wager_sol == 0.0;
    let can_create = !matches!(lobby.status, LobbyStatus::Pending)
        && (is_free_game
            || (wallet_connected
                && lobby.wager_sol > 0.0
                && (lobby.wager_sol as f64) <= balance - 0.002))
        && !matches!(lobby.status, LobbyStatus::Pending);

    let is_devnet = lobby.cached_rpc_url.contains("devnet");
    
    if ui.add_sized([ui.available_width(), 40.0], egui::Button::new(
        egui::RichText::new(if is_free_game { "🎮 Host Free Game" } else { "🎮 Create Wagered Game" })
            .size(16.0)
            .strong()
    ).fill(if can_create { egui::Color32::from_rgb(40, 100, 40) } else { egui::Color32::from_rgb(40, 40, 40) }))
    .clicked() && can_create {
        if is_free_game {
            // Free game: announce to VPS relay with stake 0.0, then go straight into game.
            // No Solana transaction needed.
            let game_id = format!("free_{}", rand::random::<u32>());
            let display_name = lobby
                .cached_keypair_bytes
                .as_ref()
                .map(|b| {
                    // Show first 6 chars of hex as a display name.
                    let hex: String = b.iter().take(3).map(|x| format!("{:02x}", x)).collect();
                    format!("Player {}", &hex)
                })
                .unwrap_or_else(|| "Anonymous".to_string());

            match crate::multiplayer::vps_client::p2p_announce_game(
                game_id.clone(),
                "unknown_node_id", // node_id not needed for free lobby listing
                &display_name,
                0.0,
                "P2P",
                10,
                Some(display_name.clone()), // username from player identity
                None, // ELO - will be fetched from profile in future
                None, // Region - will be fetched from profile in future
            ) {
                Ok(()) => {
                    info!("[LOBBY] Free game announced (id={}). Waiting for opponent via VPS.", game_id);
                    lobby.status = LobbyStatus::WaitingForOpponent { game_id: 0 };
                }
                Err(e) => {
                    warn!("[LOBBY] VPS announce failed ({}). Starting locally anyway.", e);
                    lobby.status = LobbyStatus::WaitingForOpponent { game_id: 0 };
                }
            }
        } else {
            // Wagered game: enforce CARF compliance on mainnet, then spawn Solana TX.
            if !is_devnet && compliance.status != crate::ui::compliance_modal::SubmissionStatus::Success {
                compliance.show = true;
                if let Some(wallet_pubkey) = wallet_pubkey_from_cached(&lobby.cached_keypair_bytes) {
                    compliance.pubkey = Some(wallet_pubkey.to_string());
                }
            } else if let Some(wallet_pubkey) = wallet_pubkey_from_cached(&lobby.cached_keypair_bytes) {
                let (tx, rx) = tokio::sync::oneshot::channel();
                spawn_create_game(
                    lobby.cached_rpc_url.clone(),
                    wallet_pubkey,
                    lobby.wager_lamports(),
                    tx,
                );
                lobby.tx_rx = Some(rx);
                lobby.status = LobbyStatus::Pending;
                info!("[SOLANA_LOBBY] Creating wagered game ({} SOL)", lobby.wager_sol);
            }
        }
    }

    if !can_create && wallet_connected && balance < 0.003 {
        ui.colored_label(egui::Color32::RED, "Insufficient balance (need ≥ 0.003 SOL)");
    }
}

#[cfg(feature = "solana")]
fn render_join_tab(
    ui: &mut egui::Ui,
    lobby: &mut crate::multiplayer::solana::lobby::SolanaLobbyState,
) {
    ui.label(egui::RichText::new("Enter Game ID:").size(14.0));
    ui.text_edit_singleline(&mut lobby.game_id_input);

    Layout::small_space(ui);

    let game_id_valid = lobby.game_id_input.trim().parse::<u64>().is_ok();
    let looking_up = matches!(lobby.status, LobbyStatus::Pending);
    let already_fetched = matches!(lobby.status, LobbyStatus::Fetched { .. });

    // Auto-lookup if pre-filled but not yet looked up
    if game_id_valid && !looking_up && !already_fetched && !lobby.game_id_input.is_empty() {
        if let Ok(game_id) = lobby.game_id_input.trim().parse::<u64>() {
            let (tx, rx) = tokio::sync::oneshot::channel();
            spawn_lookup_game(lobby.cached_rpc_url.clone(), game_id, tx);
            lobby.lookup_rx = Some(rx);
            lobby.status = LobbyStatus::Pending;
            info!("[SOLANA_LOBBY] Automatic lookup for game {}", game_id);
        }
    }

    if ui.add_sized([ui.available_width(), 30.0], egui::Button::new("🔍 Manual Look Up"))
        .clicked() && game_id_valid && !looking_up {
        if let Ok(game_id) = lobby.game_id_input.trim().parse::<u64>() {
            let (tx, rx) = tokio::sync::oneshot::channel();
            spawn_lookup_game(lobby.cached_rpc_url.clone(), game_id, tx);
            lobby.lookup_rx = Some(rx);
            lobby.status = LobbyStatus::Pending;
            info!("[SOLANA_LOBBY] Manual lookup for game {}", game_id);
        }
    }

    // Show fetched wager info + confirm join button
    if let LobbyStatus::Fetched { wager_sol, game_id } = lobby.status {
        ui.separator();
        Layout::small_space(ui);
        ui.label(
            egui::RichText::new(format!("Game #{} requires {:.4} SOL wager", game_id, wager_sol))
                .color(egui::Color32::GOLD),
        );
        ui.label(
            egui::RichText::new(format!("Your balance: {:.4} SOL", lobby.cached_balance))
                .size(12.0),
        );

        let sufficient = lobby.cached_balance >= wager_sol + 0.002;
        let can_join = lobby.cached_keypair_bytes.is_some() && sufficient;

        if !sufficient {
            ui.colored_label(egui::Color32::RED, "Insufficient balance to join");
        }

        if ui.add_enabled(can_join, egui::Button::new("✅ Confirm Join")).clicked() {
            if let Some(wallet_pubkey) = wallet_pubkey_from_cached(&lobby.cached_keypair_bytes) {
                let (tx, rx) = tokio::sync::oneshot::channel();
                spawn_join_game(
                    lobby.cached_rpc_url.clone(),
                    wallet_pubkey,
                    game_id,
                    tx,
                );
                // Copy fetched wager into create flow so poll_lobby_tasks can persist it
                lobby.wager_sol = wager_sol as f32;
                lobby.tx_rx = Some(rx);
                lobby.status = LobbyStatus::Pending;
                info!("[SOLANA_LOBBY] Joining game {} (wager {:.4} SOL)", game_id, wager_sol);
            }
        }
    }
}

#[cfg(feature = "solana")]
fn wallet_pubkey_from_cached(bytes: &Option<Vec<u8>>) -> Option<solana_sdk::pubkey::Pubkey> {
    let arr: [u8; 32] = bytes.as_deref()?.try_into().ok()?;
    Some(solana_sdk::pubkey::Pubkey::from(arr))
}

/// System to render a popup asking if the user wants to create a regular P2P lobby or a Solana wager lobby.
pub fn render_lobby_selection_popup(
    mut contexts: bevy_egui::EguiContexts,
    mut menu_state: ResMut<NextState<crate::core::MenuState>>,
    #[cfg(feature = "solana")]
    solana_state: Option<Res<crate::multiplayer::solana::integration::state::SolanaIntegrationState>>,
) {
    info!("[MENU] Rendering LobbySelection popup");
    let Some(ctx) = contexts.ctx_mut().ok() else { return };
    
    egui::Window::new("Create Multiplayer Lobby")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(320.0)
        .frame(
            egui::Frame::default()
                .fill(egui::Color32::from_rgb(15, 15, 20))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(230, 57, 70)))
                .corner_radius(12.0)
                .inner_margin(24.0),
        )
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Choose Lobby Type")
                    .size(20.0)
                    .color(egui::Color32::from_rgb(230, 57, 70))
                    .strong());
                
                ui.add_space(12.0);
                ui.label(egui::RichText::new("Select how you want to play against others.")
                    .color(egui::Color32::GRAY)
                    .size(13.0));
                
                ui.add_space(32.0);

                // Local Button
                let reg_btn = ui.add_sized(
                    [240.0, 48.0],
                    egui::Button::new(
                        egui::RichText::new("Local").size(16.0).strong()
                    )
                    .fill(egui::Color32::from_rgb(40, 40, 50))
                ).on_hover_text("Standard match without on-chain wagering");

                if reg_btn.clicked() {
                    menu_state.set(crate::core::MenuState::BraidLobby);
                }
                
                ui.add_space(16.0);
                
                // Solana Wager P2P Button (only available with solana feature)
                #[cfg(feature = "solana")]
                {
                    let solana_available = solana_state.as_ref().map(|s| s.wallet_pubkey.is_some()).unwrap_or(false);
                    let sol_btn = ui.add_sized(
                        [240.0, 48.0],
                        egui::Button::new(egui::RichText::new("Solana Wager P2P").size(16.0).strong())
                            .fill(if solana_available { egui::Color32::from_rgb(230, 57, 70) } else { egui::Color32::from_rgb(80, 80, 80) })
                    ).on_hover_text(if solana_available { "Play for SOL on-chain via Solana Devnet" } else { "Solana not available - feature disabled" });

                    if sol_btn.clicked() && solana_available {
                        if let Some(state) = solana_state.as_ref() {
                            if state.wallet_pubkey.is_none() {
                                info!("[MENU] Solana wager blocked — wallet not connected. Opening sign-in.");
                                if let Err(e) = webbrowser::open("http://localhost:7454/auth/login") {
                                    warn!("[MENU] Failed to open sign-in page: {}", e);
                                }
                            } else if state.profile_status != crate::multiplayer::solana::integration::state::ProfileStatus::HasProfileWithUsername {
                                info!("[MENU] Profile missing or incomplete. Redirecting to Profile Creation.");
                                menu_state.set(crate::core::MenuState::ProfileCreation);
                            } else {
                                menu_state.set(crate::core::MenuState::SolanaLobby);
                            }
                        }
                    }
                }
                
                #[cfg(not(feature = "solana"))]
                {
                    ui.add_enabled_ui(false, |ui| {
                        ui.add_sized(
                            [240.0, 48.0],
                            egui::Button::new(egui::RichText::new("Solana Wager P2P (Disabled)").size(16.0).strong())
                                .fill(egui::Color32::from_rgb(80, 80, 80))
                        ).on_hover_text("Solana feature not enabled in this build");
                    });
                }
                
                ui.add_space(32.0);
                
                if ui.button(egui::RichText::new("Cancel").color(egui::Color32::GRAY)).clicked() {
                    menu_state.set(crate::core::MenuState::Main);
                }
                
                ui.add_space(8.0);
            });
        });
}

/// Render join lobby popup
fn render_join_lobby_popup(
    ctx: &egui::Context,
    ctx_menu: &mut MainMenuUIContext,
) {
    let accent_color = egui::Color32::from_rgb(173, 92, 47); // #ad5c2f

    egui::Window::new("Join a Lobby")
        .collapsible(false)
        .resizable(false)
        .fixed_size(egui::Vec2::new(500.0, 320.0))
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .title_bar(false)
        .frame(egui::Frame {
            fill: egui::Color32::from_rgba_unmultiplied(30, 30, 30, 240),
            corner_radius: egui::Rounding::same(4),
            stroke: egui::Stroke::new(2.0, BEZEL_GREY),
            inner_margin: egui::Margin::same(16),
            ..Default::default()
        })
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Join a Lobby")
                        .size(18.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("X").clicked() {
                        ctx_menu.competitive_menu.show_join_popup = false;
                    }
                });
            });

            ui.add_space(12.0);

            if let Some(vps_state) = ctx_menu.p2p_vps_state.as_ref() {
                if !vps_state.cached_games.is_empty() {
                    for listing in &vps_state.cached_games {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new(&listing.display_name)
                                        .size(14.0)
                                        .color(egui::Color32::WHITE)
                                        .strong(),
                                );
                                let prize = if listing.stake_amount > 0.0 {
                                    format!("{:.3} SOL", listing.stake_amount)
                                } else {
                                    "Free".to_string()
                                };
                                ui.label(
                                    egui::RichText::new(prize)
                                        .size(12.0)
                                        .color(egui::Color32::from_rgb(150, 200, 150)),
                                );
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui
                                    .add_sized(
                                        [80.0, 28.0],
                                        egui::Button::new(
                                            egui::RichText::new("Join")
                                                .size(12.0)
                                                .color(egui::Color32::WHITE)
                                                .strong(),
                                        )
                                        .fill(egui::Color32::from_rgb(100, 200, 100))
                                        .corner_radius(6.0)
                                        .stroke(egui::Stroke::new(
                                            1.0,
                                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30),
                                        )),
                                    )
                                    .clicked()
                                {
                                    info!("[MENU] Joining lobby: {}", listing.game_id);
                                    // Implement actual join logic here in future; for now, close popup
                                    ctx_menu.competitive_menu.show_join_popup = false;
                                }
                            });
                        });
                        ui.add_space(8.0);
                    }
                } else {
                    ui.label(
                        egui::RichText::new("No open lobbies available.")
                            .size(14.0)
                            .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180)),
                    );
                }
            } else {
                ui.label(
                    egui::RichText::new("No lobby data available.")
                        .size(14.0)
                        .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180)),
                );
            }
        });
}
