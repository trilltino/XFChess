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
#[cfg(feature = "solana")]
use crate::multiplayer::solana::lobby::{
    spawn_create_game, spawn_join_game, spawn_lookup_game, spawn_poll_opponent_joined,
    LobbyMode, LobbyStatus,
};
use crate::ui::styles::{Layout, *};
use crate::ui::system_params::MainMenuUIContext;
use bevy::prelude::*;
use bevy_egui::{egui, EguiPrimaryContextPass};


/// Plugin for main menu state
pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        use super::main_menu_showcase::{
            animate_showcase_captures, animate_showcase_idle_float, animate_showcase_pieces,
            restart_showcase_when_complete, run_showcase_game, spawn_showcase_board,
            spawn_showcase_pieces, ShowcaseGameState,
        };

        // Common systems for all platforms
        app.add_systems(
            OnEnter(GameState::MainMenu),
            (
                setup_menu_camera,
                spawn_showcase_board,
                spawn_showcase_pieces,
                start_asset_loading,
            ),
        )
            .init_resource::<PlayerColorChoice>()
        .init_resource::<ShowcaseGameState>()
        .init_resource::<crate::assets::GameAssets>()
        .init_resource::<crate::assets::LoadingProgress>()
        .init_resource::<crate::assets::AssetLoadingTimer>()
        .init_resource::<CompetitiveMenuState>()
        .init_resource::<crate::states::tournament_menu::TournamentLobbyState>()
        .add_systems(
            EguiPrimaryContextPass,
            (
                main_menu_ui_wrapper.run_if(in_state(GameState::MainMenu)),
                #[cfg(feature = "solana")]
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
                run_showcase_game,
                animate_showcase_pieces,
                animate_showcase_captures,
                animate_showcase_idle_float,
                restart_showcase_when_complete,
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
    // but assuming PieceViewer is handled elsewhere or sharing this?)
    // Actually, PieceViewer is a substate of MainMenu, so it might need its own UI or exit button.
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

/// Render website-style main menu with navbar and sections
fn render_website_menu(ctx: &egui::Context, ctx_menu: &mut MainMenuUIContext) {
    // Show loading screen if assets aren't loaded yet
    if !ctx_menu.loading_progress.complete {
        render_loading_screen_website(ctx, ctx_menu);
        return;
    }
    
    let screen_rect = ctx.content_rect();
    
    // === BLACK GRADIENT BACKGROUND ===
    egui::CentralPanel::default()
        .frame(egui::Frame {
            fill: egui::Color32::from_rgb(0, 0, 0), // Pure black
            ..Default::default()
        })
        .show(ctx, |ui| {
            ui.add_space(0.0); // Just fill the background
        });
    
    // === NAVBAR ===
    render_navbar(ctx, ctx_menu);
    

    
    // === MAIN CONTENT AREA ===
    let content_top = 80.0; // Space below navbar
    let content_height = screen_rect.height() - content_top - 20.0;
    
    egui::Area::new("main_content".into())
        .anchor(egui::Align2::LEFT_TOP, egui::Vec2::new(20.0, content_top))
        .show(ctx, |ui| {
            ui.set_width(screen_rect.width() - 40.0);
            ui.set_height(content_height);
            
            // Two column layout
            let available_w = screen_rect.width() - 40.0;
            let col_spacing = 40.0;
            let inner_w = available_w - col_spacing;
            
            ui.horizontal(|ui| {
                // === LEFT COLUMN: TOURNAMENTS & MODES ===
                ui.vertical(|ui| {
                    ui.set_width(inner_w * 0.30);
                    render_tournaments_section(ui, ctx_menu);
                });
                
                ui.add_space(col_spacing);
                
                // === RIGHT COLUMN: QUICK PAIRING & LOBBY ===
                ui.vertical(|ui| {
                    let mid_w = inner_w * 0.70;
                    ui.set_width(mid_w);
                    ui.horizontal(|ui| {
                        // Quick Pairing
                        ui.vertical(|ui| {
                            ui.set_width((mid_w - col_spacing) * 0.45);
                            render_quick_pairing_section(ui, ctx_menu);
                        });
                        
                        ui.add_space(col_spacing);
                        
                        // Lobby
                        ui.vertical(|ui| {
                            ui.set_width((mid_w - col_spacing) * 0.55);
                            render_lobby_section(ui, ctx_menu);
                        });
                    });
                });
            });
        });
}

/// Render website-style navbar
fn render_navbar(ctx: &egui::Context, _ctx_menu: &mut MainMenuUIContext) {
    
    egui::TopBottomPanel::top("navbar")
        .frame(egui::Frame {
            fill: egui::Color32::from_rgba_unmultiplied(20, 20, 20, 240), // Dark with transparency
            inner_margin: egui::Margin::symmetric(20, 15),
            ..Default::default()
        })
        .show_separator_line(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // === LEFT SIDE: PLAY | WATCH | COMMUNITY | SOURCE CODE ===
                ui.horizontal(|ui| {
                    if nav_link(ui, "Play") {
                        // Handle Play navigation - scroll to main content
                        info!("[MENU] Play clicked - navigating to main game options");
                        // TODO: Implement smooth scroll to main content
                    }
                    ui.add_space(30.0);
                    if nav_link(ui, "Watch") {
                        // Handle Watch navigation - placeholder for spectating feature
                        info!("[MENU] Watch clicked - opening spectating placeholder");
                        // TODO: Implement spectating interface
                    }
                    ui.add_space(30.0);
                    if nav_link(ui, "Community") {
                        // Handle Community navigation - placeholder for community features
                        info!("[MENU] Community clicked - opening community placeholder");
                        // TODO: Implement community features (forums, chat, etc.)
                    }
                    ui.add_space(30.0);
                    if nav_link(ui, "Source Code") {
                        // Open GitHub repository
                        info!("[MENU] Source Code clicked - opening GitHub");
                        if let Err(e) = webbrowser::open("https://github.com/trilltino/XFChess") {
                            warn!("[MENU] Failed to open GitHub repository: {}", e);
                        }
                    }
                });
                
            });
        });
}

/// Render left tournaments section
fn render_tournaments_section(ui: &mut egui::Ui, ctx_menu: &mut MainMenuUIContext) {
    ui.heading(
        egui::RichText::new("PLAY")
            .size(18.0)
            .color(egui::Color32::WHITE)
            .strong(),
    );
    ui.add_space(15.0);

    // "Create a Lobby" goes straight to the unified lobby form.
    // Wallet is NOT required here — free games work without one.
    // Wagered games inside that form enforce the wallet requirement.
    let lobby_btn_resp = ui.add_sized(
        [ui.available_width(), 36.0],
        egui::Button::new(
            egui::RichText::new("+ Create a Lobby")
                .size(15.0)
                .color(egui::Color32::WHITE)
                .strong(),
        )
        .fill(egui::Color32::from_rgba_unmultiplied(55, 55, 55, 200)),
    );

    if lobby_btn_resp.clicked() {
        // Reset lobby form to Create mode.
        #[cfg(feature = "solana")]
        if let Some(ref mut lobby) = ctx_menu.solana_lobby {
            lobby.mode = crate::multiplayer::solana::lobby::LobbyMode::Create;
            lobby.status = crate::multiplayer::solana::lobby::LobbyStatus::Idle;
            lobby.wager_sol = 0.0; // default to free
        }
        ctx_menu.menu_state.set(crate::core::MenuState::SolanaLobby);
    }

    ui.add_space(8.0);

    // Play the Computer Section
    ui.group(|ui| {
        ui.set_width(ui.available_width());
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("V S   S T O C K F I S H")
                        .size(14.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(ctx_menu.ai_config.difficulty.description())
                            .size(11.0)
                            .color(egui::Color32::from_rgb(150, 150, 150)),
                    );
                });
            });
            
            ui.add_space(8.0);
            
            // Difficulty Slider (Level 1-8)
            let mut diff_val = ctx_menu.ai_config.difficulty.to_u8();
            let slider = ui.add(
                egui::Slider::new(&mut diff_val, 1..=8)
                    .show_value(false)
                    .trailing_fill(true)
            );
            
            if slider.changed() {
                ctx_menu.ai_config.difficulty = crate::game::ai::resource::AIDifficulty::from_u8(diff_val);
            }
            
            ui.add_space(10.0);
            
            if ui.add_sized(
                [ui.available_width(), 32.0],
                egui::Button::new(
                    egui::RichText::new("START GAME")
                        .size(12.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                )
                .fill(egui::Color32::from_rgba_unmultiplied(60, 60, 60, 200)),
            ).clicked() {
                info!("[MENU] Play the Computer clicked at level {}", diff_val);
                ctx_menu.ai_config.mode = GameMode::VsAI { 
                    ai_color: crate::rendering::pieces::PieceColor::Black 
                };
                *ctx_menu.core_mode = CoreGameMode::SinglePlayer;
                ctx_menu.next_state.set(GameState::InGame);
            }
        });
    });

    ui.add_space(30.0);
    ui.separator();
    ui.add_space(30.0);

    ui.heading(
        egui::RichText::new("OPEN TOURNAMENTS")
            .size(18.0)
            .color(egui::Color32::WHITE)
            .strong(),
    );
    ui.add_space(15.0);
    
    // Dynamic tournament listings from VPS cached data
    let mut tournaments_found = false;
    if let Some(vps_state) = ctx_menu.p2p_vps_state.as_ref() {
        for listing in &vps_state.cached_games {
            if listing.game_type == "tournament" {
                tournaments_found = true;
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(&listing.display_name)
                                .size(14.0)
                                .color(egui::Color32::WHITE)
                                .strong(),
                        );
                        let tag = if listing.stake_amount > 0.0 {
                            format!("{:.3} SOL wager", listing.stake_amount)
                        } else {
                            "Free".to_string()
                        };
                        ui.label(
                            egui::RichText::new(tag)
                                .size(11.0)
                                .color(egui::Color32::from_rgb(150, 200, 150)),
                        );
                        ui.add_space(5.0);
                        if bezel_button_compact(ui, "Join", egui::Color32::from_rgb(100, 200, 100)) {
                            ctx_menu.next_state.set(GameState::InGame);
                        }
                    });
                });
                ui.add_space(8.0);
            }
        }
    }

    if !tournaments_found {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label(
                egui::RichText::new("No active tournaments found.")
                    .size(12.0)
                    .color(egui::Color32::from_rgb(100, 100, 100))
                    .italics(),
            );
        });
    }
}

/// Render middle quick pairing section
fn render_quick_pairing_section(ui: &mut egui::Ui, ctx_menu: &mut MainMenuUIContext) {
    ui.heading(
        egui::RichText::new("QUICK PLAY")
            .size(18.0)
            .color(egui::Color32::WHITE)
            .strong(),
    );
    ui.add_space(15.0);

    // Local PvP — always functional
    if ui.add_sized(
        [ui.available_width(), 40.0],
        egui::Button::new(
            egui::RichText::new("♟  Local PvP")
                .size(14.0)
                .color(egui::Color32::WHITE)
                .strong(),
        )
        .fill(egui::Color32::from_rgba_unmultiplied(50, 80, 50, 200)),
    ).clicked() {
        info!("[MENU] Starting Local PvP game");
        ctx_menu.ai_config.mode = GameMode::Multiplayer;
        *ctx_menu.core_mode = CoreGameMode::SinglePlayer;
        ctx_menu.next_state.set(GameState::InGame);
    }
    ui.add_space(8.0);

    // Online wager tiers (locked until wallet connected)
    let wagers = [("£2 Wager", 0.05), ("£5 Wager", 0.12), ("£10 Wager", 0.25)];
    for (name, _stake) in wagers {
        let btn_text = format!("{} 🔒", name);
        let resp = ui.add_sized(
            [ui.available_width(), 36.0],
            egui::Button::new(
                egui::RichText::new(btn_text)
                    .size(13.0)
                    .color(egui::Color32::from_rgb(120, 120, 120))
                    .strong(),
            )
            .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 150)),
        ).on_hover_text("Connect wallet to wager");

        if resp.clicked() {
            info!("[MENU] Wager {} clicked — wallet not connected, opening sign-in", name);
            let _ = webbrowser::open("http://localhost:7454/auth/login");
        }
        ui.add_space(5.0);
    }
}

/// Render middle lobby section with live VPS listings and type filter.
fn render_lobby_section(ui: &mut egui::Ui, ctx_menu: &mut MainMenuUIContext) {
    ui.heading(
        egui::RichText::new("LOBBY")
            .size(18.0)
            .color(egui::Color32::WHITE)
            .strong(),
    );
    ui.add_space(10.0);

    // Filter tabs: All | Free | Wagered
    ui.horizontal(|ui| {
        let filter = &mut ctx_menu.competitive_menu.lobby_filter;
        let active_col = egui::Color32::WHITE;
        let inactive_col = egui::Color32::from_rgb(120, 120, 120);

        for (label, variant) in &[
            ("All", LobbyFilter::All),
            ("Free", LobbyFilter::Free),
            ("Wagered", LobbyFilter::Wagered),
        ] {
            let selected = *filter == *variant;
            let text = egui::RichText::new(*label)
                .size(12.0)
                .color(if selected { active_col } else { inactive_col })
                .strong();
            if ui.selectable_label(selected, text).clicked() {
                *filter = *variant;
            }
            ui.add_space(8.0);
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("🔄").on_hover_text("Refresh game list").clicked() {
                // Reset poll timer so the next frame triggers a new fetch
                if let Some(ref mut vps) = ctx_menu.p2p_vps_state {
                    vps.last_poll = None;
                }
                info!("[LOBBY] Refresh requested");
            }
        });
    });

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);

    // Collect matching games from VPS cache
    let filter = ctx_menu.competitive_menu.lobby_filter;
    let mut any_game = false;

    if let Some(vps_state) = ctx_menu.p2p_vps_state.as_ref() {
        // Clone listings to avoid holding an immutable borrow of ctx_menu while
        // we need a mutable borrow of it inside the group closures.
        let listings: Vec<_> = vps_state
            .cached_games
            .iter()
            .filter(|g| g.game_type != "tournament")
            .filter(|g| match filter {
                LobbyFilter::All => true,
                LobbyFilter::Free => g.stake_amount <= 0.0,
                LobbyFilter::Wagered => g.stake_amount > 0.0,
            })
            .cloned()
            .collect();

        // Extract the transmitter before the loop
        let tx_channel = ctx_menu.p2p_vps_state.as_ref().map(|v| v.response_tx.clone());

        for listing in listings {
            any_game = true;
            let is_wagered = listing.stake_amount > 0.0;
            let stake_tag = if is_wagered {
                format!("{:.3} SOL", listing.stake_amount)
            } else {
                "Free".to_string()
            };
            let stake_col = if is_wagered {
                egui::Color32::from_rgb(255, 200, 80)
            } else {
                egui::Color32::from_rgb(100, 200, 150)
            };

            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(&listing.display_name)
                                .size(13.0)
                                .color(egui::Color32::WHITE)
                                .strong(),
                        );
                        ui.label(
                            egui::RichText::new(&stake_tag)
                                .size(11.0)
                                .color(stake_col),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let join_col = if is_wagered {
                            egui::Color32::from_rgb(255, 160, 60)
                        } else {
                            egui::Color32::from_rgb(80, 200, 120)
                        };
                        if bezel_button_compact(ui, "Join", join_col) {
                            info!("[LOBBY] Joining game: {}", listing.game_id);
                            
                            if let Some(ref tx) = tx_channel {
                                let game_id = listing.game_id.clone();
                                let stake_amount = listing.stake_amount;
                                let wagered = is_wagered;
                                let tx = tx.clone();
                                bevy::tasks::IoTaskPool::get()
                                    .spawn(async move {
                                        match crate::multiplayer::vps_client::p2p_join_game(
                                            game_id.clone(),
                                            "unknown_node_id",
                                        ) {
                                            Ok(Some(host_id)) => {
                                                let stake = if wagered { stake_amount } else { 0.0 };
                                                let _ = tx.send(
                                                    crate::multiplayer::network::p2p_vps::VpsResponse::JoinResult {
                                                        game_id,
                                                        host_node_id: Some(host_id),
                                                        stake_amount: stake,
                                                    },
                                                );
                                            }
                                            Ok(None) => {
                                                let _ = tx.send(
                                                    crate::multiplayer::network::p2p_vps::VpsResponse::Error(
                                                        "Game rejected or full".to_string(),
                                                    ),
                                                );
                                            }
                                            Err(_e) => {
                                                let _ = tx.send(
                                                    crate::multiplayer::network::p2p_vps::VpsResponse::Error(
                                                        format!("Join failed: {_e}"),
                                                    ),
                                                );
                                            }
                                        }
                                    })
                                    .detach();
                            }
                        }
                    });
                });
            });
            ui.add_space(6.0);
        }
    }

    if !any_game {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label(
                egui::RichText::new("No public games available")
                    .size(12.0)
                    .color(egui::Color32::from_rgb(100, 100, 100))
                    .italics(),
            );
            ui.label(
                egui::RichText::new("Host a lobby or play locally")
                    .size(11.0)
                    .color(egui::Color32::from_rgb(80, 80, 80)),
            );
        });
    }
}


/// Helper to transition to Solana Lobby for hosting a specific wager
// Temporarily disabled to remove lightyear dependencies
/*
fn host_wager_lobby(ctx: &mut MainMenuUIContext, sol_amount: f32) {
    #[cfg(feature = "solana")]
    {
        let wallet_connected = ctx.wallet.as_ref().map(|w| w.is_connected()).unwrap_or(false);
        if wallet_connected {
            ctx.menu_state.set(crate::core::MenuState::SolanaLobby);
            if let Some(lobby) = ctx.solana_lobby.as_mut() {
                lobby.wager_sol = sol_amount;
                lobby.mode = crate::multiplayer::solana::lobby::LobbyMode::Create;
                lobby.status = crate::multiplayer::solana::lobby::LobbyStatus::Idle;
            }
        } else {
            info!("[MENU] Wager hosting blocked — wallet not connected. Opening sign-in.");
            if let Err(e) = webbrowser::open("http://localhost:7454/auth/login") {
                warn!("[MENU] Failed to open sign-in page: {}", e);
            }
        }
    }
}
*/

/// Navbar link helper
fn nav_link(ui: &mut egui::Ui, text: &str) -> bool {
    let response = ui.label(
        egui::RichText::new(text)
            .size(14.0)
            .color(egui::Color32::from_rgb(200, 200, 200)),
    );
    
    if response.hovered() {
        ui.painter().rect(
            response.rect,
            0.0,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 20),
            egui::Stroke::NONE,
            egui::epaint::StrokeKind::Middle,
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
            egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 150)),
            egui::epaint::StrokeKind::Middle,
        );
    }
    
    response
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
            // *ctx.core_mode = CoreGameMode::MultiplayerLocal; // Temporarily disabled
            ctx.next_state.set(GameState::InGame);
            info!("[MAIN_MENU] Starting Local PvP game");
        }

        Layout::small_space(ui);

        // Temporarily disabled to remove lightyear dependencies
        /*
        ui.horizontal(|ui| {
            ui.label("Play as:");
            if ui.selectable_label(ctx.color_choice.play_as_white, "♔ White").clicked() {
                ctx.color_choice.play_as_white = true;
            }
            if ui.selectable_label(!ctx.color_choice.play_as_white, "♚ Black").clicked() {
                ctx.color_choice.play_as_white = false;
            }
        });
        */

        Layout::small_space(ui);

        // Temporarily disabled to remove lightyear dependencies
        let ai_color = crate::rendering::pieces::PieceColor::Black; // Default to AI as Black

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
        // Temporarily disabled to remove lightyear dependencies
        let vps_status = if false { // ctx.settings.use_vps_relay
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

        // Get node ID from network state
        // Temporarily disabled to remove lightyear dependencies
        let node_id_display = "Initializing...".to_string();

        ui.label(
            egui::RichText::new(&node_id_display)
                .size(16.0)
                .color(egui::Color32::from_rgb(100, 200, 255))
                .monospace(),
        );

        if bezel_button(ui, "📋 Copy Full Node ID", egui::Color32::from_rgb(100, 200, 255)) {
            // if let Some(node_id) = &ctx.network_state.node_id { // Temporarily disabled
            //     let full_node_id = bs58::encode(node_id.as_bytes()).into_string();
            //     ui.output_mut(|o| {
            //         o.commands
            //             .push(egui::OutputCommand::CopyText(full_node_id.clone()))
            //     });
            //     info!("[MAIN_MENU] Node ID copied to clipboard: {}", full_node_id);
            // }
        }

        Layout::item_space(ui);
        ui.separator();
        Layout::item_space(ui);
    });
}

// --- P2P LOBBY UI ---
// Temporarily disabled to remove lightyear dependencies
/*
fn ui_p2p_lobby(ui: &mut egui::Ui, ctx: &mut MainMenuUIContext) {
    ui.vertical_centered(|ui| {
        Layout::section_space(ui);

        if ui.button("⬅ Back").clicked() {
            ctx.menu_state.set(crate::core::MenuState::ModeSelect);
        }

        Layout::section_space(ui);

        ui.label(
            egui::RichText::new("BRAID P2P LOBBY")
                .size(24.0)
                .color(egui::Color32::from_rgb(100, 200, 255))
                .strong(),
        );

        Layout::item_space(ui);

        // --- HOST GAME ---
        ui.label(
            egui::RichText::new("HOST GAME")
                .size(16.0)
                .color(egui::Color32::from_rgb(100, 255, 150)),
        );
        Layout::small_space(ui);

        // Show connection status if any
        // Temporarily disabled to remove lightyear dependencies
        /*
        match ctx.p2p_state.status {
            crate::multiplayer::P2PConnectionStatus::Hosting => {
                ui.label(
                    egui::RichText::new("⏳ Waiting for peer to connect...")
                        .size(12.0)
                        .color(egui::Color32::from_rgb(100, 255, 150)),
                );
            }
            crate::multiplayer::P2PConnectionStatus::Connecting => {
                ui.label(
                    egui::RichText::new("⏳ Sending invite to host...")
                        .size(12.0)
                        .color(egui::Color32::from_rgb(255, 200, 100)),
                );
            }
            crate::multiplayer::P2PConnectionStatus::Connected => {
                if ctx.p2p_state.is_host {
                    ui.label(
                        egui::RichText::new("✅ Peer joined! Starting game...")
                            .size(14.0)
                            .color(egui::Color32::from_rgb(100, 255, 100))
                            .strong(),
                    );
                    if let Some(ref peer_id) = ctx.p2p_state.peer_node_id {
                        ui.label(
                            egui::RichText::new(format!(
                                "Opponent: {}...",
                                &peer_id[..peer_id.len().min(16)]
                            ))
                                .size(11.0)
                                .color(egui::Color32::from_rgb(150, 200, 255))
                                .monospace(),
                        );
                    }
                } else {
                    ui.label(
                        egui::RichText::new("✅ Host accepted! Game starting...")
                            .size(14.0)
                            .color(egui::Color32::from_rgb(100, 255, 100))
                            .strong(),
                    );
                }
            }
            crate::multiplayer::P2PConnectionStatus::Error(ref msg) => {
                ui.label(
                    egui::RichText::new(format!("❌ {}", msg))
                        .size(12.0)
                        .color(egui::Color32::from_rgb(255, 80, 80)),
                );
            }
            _ => {}
        }
        */

        ui.label(
            egui::RichText::new("Lobby Name:")
                .size(14.0)
                .color(egui::Color32::from_rgb(150, 150, 150)),
        );
        ui.text_edit_singleline(&mut String::new()); // Temporarily disabled
        Layout::small_space(ui);

        if bezel_button(ui, "Start Hosting", egui::Color32::from_rgb(100, 255, 150)) {
            // Set AI mode to multiplayer
            ctx.ai_config.mode = GameMode::Multiplayer;
            // Check if VPS relay is enabled
            // Temporarily disabled to remove lightyear dependencies
            /*
            if ctx.settings.use_vps_relay {
                // if let Some(node_id) = &ctx.network_state.node_id { // Temporarily disabled
                //     let node_id_str = bs58::encode(node_id.as_bytes()).into_string();
                    let game_id = format!("p2p_{}", rand::random::<u64>());
                    let display_name = "Guest Player".to_string(); // Temporarily disabled

                    match crate::multiplayer::vps_client::p2p_announce_game(
                        game_id.clone(),
                        &node_id_str,
                        &display_name,
                        0.0,
                        "P2P",
                        10,
                    ) {
                        Ok(()) => {
                            info!("[MAIN_MENU] Hosted game '{}' (ID: {}) via VPS", display_name, game_id);
                            // ctx.p2p_ui // Temporarily disabled.clear_error();
                            // Enable VPS relay mode
                            // if let Some(ref mut vps_state) = ctx.p2p_vps_state { // Temporarily disabled
                            //     crate::multiplayer::network::p2p_vps::set_vps_relay_mode(vps_state, true);
                            // }
                        }
                        Err(e) => {
                            // ctx.p2p_ui // Temporarily disabled.set_error(format!("VPS announce failed: {}", e));
                            return;
                        }
                    }
                }
            }
            */
            if false { // Temporarily disabled
                // Direct P2P - disable VPS relay
                // Temporarily disabled to remove lightyear dependencies
                /*
                if let Some(ref mut vps_state) = ctx.p2p_vps_state {
                    crate::multiplayer::network::p2p_vps::set_vps_relay_mode(vps_state, false);
                }
                */
                // Emit host game event
                // ctx.host_game_events.write(crate::multiplayer::HostGameEvent); // Temporarily disabled
            }
            *ctx.core_mode = CoreGameMode::BraidMultiplayer;
            // ctx.p2p_state.status = crate::multiplayer::P2PConnectionStatus::Hosting; // Temporarily disabled
            info!("[MAIN_MENU] Hosting P2P game");
        }

        ui.label(
            egui::RichText::new(if ctx.settings.use_vps_relay {
                "Game will be listed on VPS relay"
            } else {
                "Wait for a peer to connect using your Node ID"
            })
                .size(12.0)
                .color(egui::Color32::from_rgb(150, 150, 150)),
        );

        Layout::item_space(ui);
        ui.separator();
        Layout::item_space(ui);

        // --- JOIN GAME ---
        ui.label(
            egui::RichText::new("JOIN GAME")
                .size(16.0)
                .color(egui::Color32::from_rgb(255, 200, 100)),
        );
        Layout::small_space(ui);

        if ctx.settings.use_vps_relay {
            ui.label(
                egui::RichText::new("Enter Peer Node ID (or use VPS lobby)")
                    .size(14.0)
                    .color(egui::Color32::from_rgb(150, 150, 150)),
            );
        } else {
            ui.label(
                egui::RichText::new("Enter Peer Node ID")
                    .size(14.0)
                    .color(egui::Color32::from_rgb(150, 150, 150)),
            );
        }

        // Text input for peer node ID (persisted across frames)
        let response = ui.text_edit_singleline(&mut String::new()); // Temporarily disabled

        // Clear error when user starts typing
        if response.changed() {
            // ctx.p2p_ui // Temporarily disabled.clear_error();
        }

        // Display error message if present
        if false { // Temporarily disabled
            Layout::small_space(ui);
            ui.label(
                egui::RichText::new(format!("⚠ {}", error))
                    .size(12.0)
                    .color(egui::Color32::from_rgb(255, 80, 80)),
            );
        }

        Layout::small_space(ui);

        // Temporarily disabled to remove lightyear dependencies
        let is_connecting = false;
        let is_error = false;

        if is_connecting {
            ui.label(
                egui::RichText::new("⏳ Connecting... (up to 12s)")
                    .size(12.0)
                    .color(egui::Color32::from_rgb(255, 200, 100)),
            );
        } else {
            let btn_label = if is_error { "🔗 Retry Connect" } else { "Connect to Peer" };
            if bezel_button(ui, btn_label, egui::Color32::from_rgb(255, 200, 100)) {
                match Ok::<(), String>(()) { // Temporarily disabled
                    Ok(()) => {
                        // ctx.p2p_ui // Temporarily disabled.clear_error();
                        ctx.ai_config.mode = GameMode::Multiplayer;
                        // Check if VPS relay is enabled
                        // Temporarily disabled to remove lightyear dependencies
                        /*
                        if ctx.settings.use_vps_relay {
                            // if let Some(node_id) = &ctx.network_state.node_id { // Temporarily disabled
                                // let node_id_str = bs58::encode(node_id.as_bytes()).into_string();
                                match crate::multiplayer::vps_client::p2p_join_game(
                                    String::new(), // Temporarily disabled
                                    // &node_id_str,
                                ) {
                                    Ok(Some(_host_node_id)) => {
                                        info!("[MAIN_MENU] Joined game via VPS");
                                        // Enable VPS relay mode
                                        // if let Some(ref mut vps_state) = ctx.p2p_vps_state { // Temporarily disabled
                                        //     crate::multiplayer::network::p2p_vps::set_vps_relay_mode(vps_state, true);
                                        // }
                                    }
                                    Ok(None) => {
                                        // ctx.p2p_ui // Temporarily disabled.set_error("Game not found or rejected".to_string());
                                        return;
                                    }
                                    Err(e) => {
                                        // ctx.p2p_ui // Temporarily disabled.set_error(format!("VPS join failed: {}", e));
                                        return;
                                    }
                                }
                            }
                        }
                        */
                        if false { // Temporarily disabled
                            // Temporarily disabled to remove lightyear dependencies
                            /*
                            // Direct P2P - disable VPS relay
                            // if let Some(ref mut vps_state) = ctx.p2p_vps_state { // Temporarily disabled
                                crate::multiplayer::network::p2p_vps::set_vps_relay_mode(vps_state, false);
                            }
                            ctx.connect_events.write(crate::multiplayer::ConnectToPeerEvent {
                                peer_node_id: String::new(), // Temporarily disabled
                            });
                            */
                        } // end if false
                        // *ctx.core_mode = CoreGameMode::BraidMultiplayer; // Temporarily disabled
                        info!(
                            "[MAIN_MENU] Joining P2P game with peer: {}",
                            "" // // ctx.p2p_ui // Temporarily disabled.peer_input
                        );
                    }
                    Err(error_msg) => {
                        // // ctx.p2p_ui // Temporarily disabled.set_error(error_msg); // Temporarily disabled
                        warn!(
                            "[MAIN_MENU] Invalid Node ID entered: {}",
                            "" // // ctx.p2p_ui // Temporarily disabled.peer_input
                        );
                    }
                }
            }
        }
    });
}
*/

// Temporarily disabled to remove lightyear dependencies
/*
fn ui_braid_lobby(ui: &mut egui::Ui, ctx: &mut MainMenuUIContext) {
    ui.vertical_centered(|ui| {
        Layout::section_space(ui);

        if ui.button("⬅ Back").clicked() {
            ctx.menu_state.set(crate::core::MenuState::ModeSelect);
        }

        Layout::section_space(ui);

        ui.label(
            egui::RichText::new("BRAID P2P LOBBY")
                .size(24.0)
                .color(egui::Color32::from_rgb(180, 120, 255))
                .strong(),
        );

        Layout::item_space(ui);

        // Temporarily disabled to remove lightyear dependencies
        /*
        ui.group(|ui| {
            ui.label("Base URL:");
            ui.text_edit_singleline(&mut ctx.braid_config.base_url);

            Layout::small_space(ui);

            ui.label("Game ID:");
            ui.text_edit_singleline(&mut ctx.braid_config.game_id);
        });

        Layout::item_space(ui);

        if ui.button("CONNECT & PLAY").clicked() {
            ctx.braid_config.active = true;
            *ctx.core_mode = CoreGameMode::BraidMultiplayer;
            ctx.next_state.set(GameState::InGame);

        }
        */

        Layout::item_space(ui);
        ui.label(
            egui::RichText::new(
                "Braid protocol uses decentralized HTTP for real-time state synchronization.",
            )
            .size(10.0)
            .color(egui::Color32::from_rgb(150, 150, 150)),
        );
    });
}
*/

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

        // Node ID display
        // Temporarily disabled to remove lightyear dependencies
        /*
        if let Some(node_id) = &ctx.network_state.node_id {
            let full = bs58::encode(node_id.as_bytes()).into_string();
            let short_id = if full.len() > 16 {
                format!("{:.16}...", full)
            } else {
                full.clone()
            };
            ui.label(
                egui::RichText::new(format!("Node ID: {}", short_id))
                    .size(12.0)
                    .color(egui::Color32::from_rgb(100, 200, 255))
                    .monospace(),
            );
            if ui.small_button("📋 Copy Node ID").clicked() {
                ui.output_mut(|o| {
                    o.commands
                        .push(egui::OutputCommand::CopyText(full.clone()));
                });
                info!("[SOLANA_LOBBY] Node ID copied: {}", full);
            }
        } else {
            ui.label(
                egui::RichText::new("Node ID: Initializing...")
                    .size(12.0)
                    .color(egui::Color32::from_rgb(150, 150, 150)),
            );
        }
        */

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
                    egui::RichText::new(format!(
                        "⏳ Game #{} — waiting for opponent to join on-chain...",
                        game_id
                    ))
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
                    // ctx.host_game_events // Temporarily disabled
                    //     .write(crate::multiplayer::HostGameEvent);
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

                let response = ui.text_edit_singleline(&mut String::new()); // Temporarily disabled
                if response.changed() {
                    // ctx.p2p_ui // Temporarily disabled.clear_error();
                }
                if let Some(ref err) = None::<String> { // Temporarily disabled
                    ui.label(
                        egui::RichText::new(format!("⚠ {}", err))
                            .size(11.0)
                            .color(egui::Color32::from_rgb(255, 80, 80)),
                    );
                }
                Layout::small_space(ui);
                if ui.button("🔗 Connect to Host").clicked() {
                    match Ok::<(), String>(()) { // Temporarily disabled
                        Ok(()) => {
                            // ctx.p2p_ui // Temporarily disabled.clear_error();
                            ctx.ai_config.mode = GameMode::Multiplayer;
                            let peer = String::new(); // Temporarily disabled
                            // ctx.connect_events // Temporarily disabled
                            //     .write(crate::multiplayer::ConnectToPeerEvent {
                            //         peer_node_id: peer.clone(),
                            //     });
                            // *ctx.core_mode = CoreGameMode::BraidMultiplayer; // Temporarily disabled
                            if let Some(ref mut sync) = ctx.solana_sync {
                                sync.game_id = Some(game_id);
                                sync.wager_amount = wager_lamports;
                            }
                            if let Some(ref mut comp) = ctx.competitive {
                                comp.game_id = Some(game_id);
                                comp.wager_lamports = wager_lamports;
                                comp.active = true;
                            }
                            info!(
                                "[SOLANA_LOBBY] Connecting to host {} for game #{}",
                                peer, game_id
                            );
                        }
                        Err(e) => {
                            // ctx.p2p_ui // Temporarily disabled.set_error(e);
                        }
                    }
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

#[cfg(feature = "solana")]
/// System to render a popup asking if the user wants to create a regular P2P lobby or a Solana wager lobby.
pub fn render_lobby_selection_popup(
    mut contexts: bevy_egui::EguiContexts,
    mut menu_state: ResMut<NextState<crate::core::MenuState>>,
    solana_state: Res<crate::multiplayer::solana::integration::state::SolanaIntegrationState>,
) {
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

                // Regular P2P Button
                let reg_btn = ui.add_sized(
                    [240.0, 48.0],
                    egui::Button::new(egui::RichText::new("Regular P2P (Free)").size(16.0).strong())
                        .fill(egui::Color32::from_rgb(40, 40, 50))
                ).on_hover_text("Standard match without on-chain wagering");

                if reg_btn.clicked() {
                    menu_state.set(crate::core::MenuState::BraidLobby);
                }
                
                ui.add_space(16.0);
                
                // Solana Wager P2P Button
                let sol_btn = ui.add_sized(
                    [240.0, 48.0],
                    egui::Button::new(egui::RichText::new("Solana Wager P2P").size(16.0).strong())
                        .fill(egui::Color32::from_rgb(230, 57, 70))
                ).on_hover_text("Play for SOL on-chain via Solana Devnet");

                if sol_btn.clicked() {
                    if solana_state.wallet_pubkey.is_none() {
                        info!("[MENU] Solana wager blocked — wallet not connected. Opening sign-in.");
                        if let Err(e) = webbrowser::open("http://localhost:7454/auth/login") {
                            warn!("[MENU] Failed to open sign-in page: {}", e);
                        }
                    } else if solana_state.profile_status != crate::multiplayer::solana::integration::state::ProfileStatus::HasProfileWithUsername {
                        info!("[MENU] Profile missing or incomplete. Redirecting to Profile Creation.");
                        menu_state.set(crate::core::MenuState::ProfileCreation);
                    } else {
                        menu_state.set(crate::core::MenuState::SolanaLobby);
                    }
                }
                
                ui.add_space(32.0);
                
                if ui.button(egui::RichText::new("Cancel").color(egui::Color32::GRAY)).clicked() {
                    menu_state.set(crate::core::MenuState::Main);
                }
                
                ui.add_space(8.0);
            });
        });
}



