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
    start_asset_loading, GameAssets, LoadingProgress,
};
use crate::core::{DespawnOnExit, GameState, PreviousState};
use crate::game::ai::{AIDifficulty, ChessAIResource, GameMode};
use crate::game::view_mode::ViewMode;
use crate::rendering::pieces::PieceColor;
use crate::ui::styles::*;
// Removed fog imports for performance - using pure black background instead
use bevy::math::ops;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use rand::Rng;

/// Plugin for main menu state
pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::MainMenu),
            (setup_menu_camera, spawn_star_spheres, start_asset_loading),
        )
        .init_resource::<MenuExpanded>() // Initialize menu state
        .add_systems(
            EguiPrimaryContextPass,
            main_menu_ui_wrapper.run_if(in_state(GameState::MainMenu)),
        )
        .add_systems(
            Update,
            (
                check_asset_loading,
                handle_asset_loading_errors,
                handle_untyped_asset_loading_errors,
                animate_menu_camera,
                ensure_menu_camera_setup, // Ensure camera is set up if it wasn't ready in OnEnter
            )
                .run_if(in_state(GameState::MainMenu)),
        );
    }
}

/// Wrapper for main_menu_ui that handles Result
fn main_menu_ui_wrapper(
    contexts: EguiContexts,
    next_state: ResMut<NextState<GameState>>,
    menu_state: ResMut<NextState<crate::core::MenuState>>,
    ai_config: ResMut<ChessAIResource>,
    previous_state: ResMut<PreviousState>,
    view_mode: ResMut<ViewMode>,
    loading_progress: ResMut<LoadingProgress>,
    game_assets: ResMut<GameAssets>,
    current_menu_state: Option<Res<State<crate::core::MenuState>>>,
    menu_expanded: ResMut<MenuExpanded>,
) {
    info!("[MAIN_MENU] UI wrapper called");
    match main_menu_ui(
        contexts,
        next_state,
        menu_state,
        ai_config,
        previous_state,
        view_mode,
        loading_progress,
        game_assets,
        current_menu_state,
        menu_expanded,
    ) {
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

/// Resource to track menu expansion state
#[derive(Resource)]
pub struct MenuExpanded {
    pub expanded: bool,
}

impl Default for MenuExpanded {
    fn default() -> Self {
        Self { expanded: false }
    }
}

/// Setup camera for main menu with pyramid scene in background
/// Uses the persistent Egui camera and updates its transform
/// Handles case where camera might not exist yet (if OnEnter runs before PreStartup)
fn setup_menu_camera(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    persistent_camera: Res<crate::PersistentEguiCamera>,
    mut camera_query: Query<
        &mut Transform,
        (With<bevy_egui::PrimaryEguiContext>, Without<MenuCamera>),
    >,
) {
    info!("[MAIN_MENU] Setting up menu camera with pyramid scene");
    info!(
        "[MAIN_MENU] DEBUG: Persistent camera entity: {:?}",
        persistent_camera.entity
    );

    // Update persistent camera transform for menu view
    // Handle gracefully if camera doesn't exist yet (OnEnter runs before PreStartup for default state)
    if let Some(camera_entity) = persistent_camera.entity {
        info!(
            "[MAIN_MENU] DEBUG: Attempting to query camera entity {:?}",
            camera_entity
        );
        match camera_query.get_mut(camera_entity) {
            Ok(mut transform) => {
                info!("[MAIN_MENU] DEBUG: Successfully queried camera transform");
                *transform = Transform::from_xyz(8.0, 12.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y);
                info!("[MAIN_MENU] Updated persistent camera transform for menu");

                // Add menu marker to persistent camera (removed VolumetricFog for performance)
                info!("[MAIN_MENU] DEBUG: Adding components to camera entity");
                commands.entity(camera_entity).insert(MenuCamera);
                info!("[MAIN_MENU] DEBUG: Components added successfully");
            }
            Err(e) => {
                error!("[MAIN_MENU] ERROR: Persistent camera entity {:?} exists but query failed: {:?}", camera_entity, e);
                error!("[MAIN_MENU] ERROR: Camera may not be ready yet. Will retry in Update.");
                error!("[MAIN_MENU] ERROR: Query filter: With<PrimaryEguiContext>, Without<MenuCamera>");
            }
        }
    } else {
        warn!("[MAIN_MENU] WARNING: Persistent camera not yet created (OnEnter runs before PreStartup). Will be set up in PreStartup.");
        warn!("[MAIN_MENU] WARNING: This is expected behavior for the default state.");
    }

    // === PYRAMID SCENE ===

    // Stone material for pyramid blocks
    let stone = materials.add(StandardMaterial {
        base_color: Srgba::hex("28221B")
            .expect("hardcoded hex color '28221B' is valid")
            .into(),
        perceptual_roughness: 0.9,
        metallic: 0.0,
        ..default()
    });

    // Four pillars surrounding the pyramid - REMOVED per user request

    // Glowing orb at the top - REMOVED per user request
    // (User wanted the green ball completely removed)

    // Pyramid steps (20 layers to reduce draw calls)
    for i in 0..20 {
        let half_size = i as f32 / 2.0 + 3.0;
        let y = -i as f32 / 2.0;
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(2.0 * half_size, 0.5, 2.0 * half_size))),
            MeshMaterial3d(stone.clone()),
            Transform::from_xyz(0.0, y + 0.25, 0.0),
            DespawnOnExit(GameState::MainMenu),
            Name::new(format!("Pyramid Layer {}", i)),
        ));
    }

    // Skybox/Background - Pure black (user requested complete black, no bluish hue)
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.0, 0.0, 0.0), // Pure black RGB(0, 0, 0)
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        Transform::from_scale(Vec3::splat(1_000_000.0)),
        DespawnOnExit(GameState::MainMenu),
        Name::new("Skybox"),
    ));

    // Rim lights on pillars - REMOVED (pillars removed)

    // Directional light for soft shadows
    commands.spawn((
        DirectionalLight {
            illuminance: 1000.0,
            shadows_enabled: true,
            color: Color::srgb(0.9, 0.9, 1.0), // Slightly blue
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            -std::f32::consts::FRAC_PI_4,
            std::f32::consts::FRAC_PI_4,
            0.0,
        )),
        DespawnOnExit(GameState::MainMenu),
        Name::new("Directional Light"),
    ));

    // Removed FogVolume for performance - using pure black background instead

    info!("[MAIN_MENU] Pyramid scene and camera setup complete");
}

/// Spawn randomly positioned star spheres with bright white lights
/// Creates a starfield effect "as far as the eye can see" in the black foggy space
fn spawn_star_spheres(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut rng = rand::thread_rng();
    // Reduced from 200 to 12 to prevent GPU memory exhaustion
    // Volumetric lights are very memory-intensive, so we use fewer stars
    let num_stars = 12;

    // White emissive material for stars
    let star_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 1.0, 1.0),        // Pure white
        emissive: LinearRgba::new(1.0, 1.0, 1.0, 1.0), // Bright white glow
        unlit: true,                                   // Always visible regardless of lighting
        ..default()
    });

    // Spawn stars across a vast area
    for i in 0..num_stars {
        // Random positions covering a large area
        let x = rng.gen_range(-150.0..150.0);
        let y = rng.gen_range(-50.0..50.0);
        let z = rng.gen_range(-150.0..150.0);

        // Random size variation for visual interest
        let radius = rng.gen_range(0.1..0.3);

        // Random light intensity variation
        let intensity = rng.gen_range(2000.0..5000.0);

        commands.spawn((
            Mesh3d(meshes.add(Sphere::new(radius))),
            MeshMaterial3d(star_material.clone()),
            PointLight {
                color: Color::srgb(1.0, 1.0, 1.0), // Bright white light
                intensity,
                range: 30.0,
                radius: 0.5,
                shadows_enabled: false, // No shadows for stars
                ..default()
            },
            // Removed VolumetricLight component to reduce GPU memory usage
            // Volumetric lights are very memory-intensive and 200 of them caused OOM errors
            Transform::from_xyz(x, y, z),
            DespawnOnExit(GameState::MainMenu),
            Name::new(format!("Star {}", i)),
        ));
    }

    info!(
        "[MAIN_MENU] Spawned {} star spheres (volumetric lights removed to prevent OOM)",
        num_stars
    );
}

/// Ensure menu camera is set up if it wasn't ready during OnEnter
/// This handles the case where OnEnter runs before PreStartup (for default state)
fn ensure_menu_camera_setup(
    persistent_camera: Res<crate::PersistentEguiCamera>,
    mut camera_query: Query<
        &mut Transform,
        (With<bevy_egui::PrimaryEguiContext>, Without<MenuCamera>),
    >,
    mut commands: Commands,
    menu_camera_query: Query<Entity, With<MenuCamera>>,
) {
    // Only set up if camera exists and menu camera marker is not present
    if menu_camera_query.is_empty() {
        debug!("[MAIN_MENU] DEBUG: MenuCamera marker not found, attempting late setup");
        if let Some(camera_entity) = persistent_camera.entity {
            debug!(
                "[MAIN_MENU] DEBUG: Persistent camera entity: {:?}",
                camera_entity
            );
            match camera_query.get_mut(camera_entity) {
                Ok(mut transform) => {
                    *transform =
                        Transform::from_xyz(8.0, 12.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y);
                    info!("[MAIN_MENU] Late setup: Updated persistent camera transform for menu");

                    commands.entity(camera_entity).insert(MenuCamera);
                    info!("[MAIN_MENU] DEBUG: Late camera setup completed successfully");
                }
                Err(e) => {
                    error!(
                        "[MAIN_MENU] ERROR: Late setup failed to query camera: {:?}",
                        e
                    );
                    error!("[MAIN_MENU] ERROR: Camera entity: {:?}", camera_entity);
                }
            }
        } else {
            warn!("[MAIN_MENU] WARNING: Late setup attempted but persistent camera entity is None");
        }
    }
}

/// Animate the camera orbiting around the pyramid
fn animate_menu_camera(mut camera_query: Query<&mut Transform, With<MenuCamera>>, time: Res<Time>) {
    let Ok(mut transform) = camera_query.single_mut() else {
        return;
    };

    let elapsed = time.elapsed_secs();

    // Orbit camera around pyramid with smooth zoom
    let orbit_scale = 8.0 + ops::sin(elapsed / 10.0) * 7.0;
    *transform = Transform::from_xyz(
        ops::cos(elapsed / 5.0) * orbit_scale,
        12.0 - orbit_scale / 2.0,
        ops::sin(elapsed / 5.0) * orbit_scale,
    )
    .looking_at(Vec3::ZERO, Vec3::Y);
}

/// Main menu UI system
fn main_menu_ui(
    mut contexts: EguiContexts,
    mut next_state: ResMut<NextState<GameState>>,
    mut menu_state: ResMut<NextState<crate::core::MenuState>>,
    mut ai_config: ResMut<ChessAIResource>,
    mut previous_state: ResMut<PreviousState>,
    mut view_mode: ResMut<ViewMode>,
    mut loading_progress: ResMut<LoadingProgress>,
    mut game_assets: ResMut<GameAssets>,
    current_menu_state: Option<Res<State<crate::core::MenuState>>>,
    mut menu_expanded: ResMut<MenuExpanded>,
) -> Result<(), bevy::ecs::query::QuerySingleError> {
    // Only show main menu UI when MenuState is Main (not PieceViewer or other substates)
    if let Some(menu_state_res) = current_menu_state {
        if *menu_state_res.get() != crate::core::MenuState::Main {
            // Not in main menu substate, don't render UI
            return Ok(());
        }
    }

    info!("[MAIN_MENU] UI system called, attempting to get context");
    let ctx = contexts.ctx_mut()?;
    info!("[MAIN_MENU] Context obtained successfully, rendering UI");

    // Top panel for collapsible menu
    egui::TopBottomPanel::top("main_menu_panel")
        .frame(egui::Frame {
            fill: egui::Color32::from_rgba_premultiplied(0, 0, 0, 200), // Semi-transparent black
            ..Default::default()
        })
        .show(ctx, |ui| -> () {
            // Toggle button always visible
            ui.horizontal(|ui| {
                ui.heading(TextStyle::heading("XFChess", TextSize::LG));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(if menu_expanded.expanded { "▲ Hide Menu" } else { "▼ Menu" }).clicked() {
                        menu_expanded.expanded = !menu_expanded.expanded;
                    }
                });
            });

            // Only show full menu if expanded
            if menu_expanded.expanded {
                ui.separator();

                // Show loading screen if assets aren't loaded yet
                if !loading_progress.complete {
                    ui.vertical_centered(|ui| {
                        Layout::small_space(ui);

                        // Check if loading failed
                        if loading_progress.failed {
                            // Error state
                            ui.heading(
                                egui::RichText::new("Asset Loading Failed")
                                    .size(20.0)
                                    .color(egui::Color32::from_rgb(220, 50, 50))
                            );

                            Layout::small_space(ui);

                            // Error message
                            if let Some(ref error_msg) = loading_progress.error_message {
                                ui.label(
                                    egui::RichText::new(error_msg)
                                        .size(12.0)
                                        .color(egui::Color32::from_rgb(220, 150, 150))
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new("Failed to load required assets")
                                        .size(12.0)
                                        .color(egui::Color32::from_rgb(220, 150, 150))
                                );
                            }

                            Layout::small_space(ui);

                            // Option to continue anyway
                            if ui.button("Continue Anyway (May cause issues)").clicked() {
                                warn!("[MAIN_MENU] User chose to continue despite asset loading failure");
                                loading_progress.complete = true;
                                loading_progress.progress = 1.0;
                                game_assets.loaded = true;
                                info!("[MAIN_MENU] Asset loading marked as complete despite failure");
                            }
                        } else {
                            // Loading state
                            ui.heading(
                                egui::RichText::new("Loading...")
                                    .size(20.0)
                                    .color(egui::Color32::from_rgb(220, 220, 220))
                            );

                            Layout::small_space(ui);

                            // Progress bar
                            let progress_bar = egui::ProgressBar::new(loading_progress.progress)
                                .desired_width(300.0)
                                .show_percentage()
                                .animate(true);

                            ui.add(progress_bar);

                            Layout::small_space(ui);

                            // Status text
                            ui.label(
                                egui::RichText::new("Loading assets...")
                                    .size(12.0)
                                    .color(egui::Color32::from_rgb(180, 180, 180))
                            );
                        }

                        Layout::small_space(ui);
                    });
                    return;
                }

                // Show full menu once assets are loaded
                ui.vertical_centered(|ui| {
                    Layout::small_space(ui);
                    ui.label(TextStyle::caption("A Modern Chess Experience"));

                    Layout::section_space(ui);

                    // === PLAY SECTION ===
                    StyledPanel::card().show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.heading(TextStyle::heading("Play Chess", TextSize::MD));

                            Layout::item_space(ui);

                            // Human vs AI (Black)
                            if StyledButton::primary(ui, "vs AI (Play as White)").clicked() {
                                ai_config.mode = GameMode::VsAI { ai_color: PieceColor::Black };
                                next_state.set(GameState::InGame);
                                menu_expanded.expanded = false; // Collapse on game start
                            }

                            Layout::small_space(ui);

                            // Human vs AI (White)
                            if StyledButton::primary(ui, "vs AI (Play as Black)").clicked() {
                                ai_config.mode = GameMode::VsAI { ai_color: PieceColor::White };
                                next_state.set(GameState::InGame);
                                menu_expanded.expanded = false;
                            }

                            Layout::small_space(ui);

                            // TempleOS View button
                            if StyledButton::primary(ui, "TempleOS View").clicked() {
                                *view_mode = ViewMode::TempleOS;
                                info!("[MAIN_MENU] TempleOS View button clicked");
                                next_state.set(GameState::InGame);
                                menu_expanded.expanded = false;
                            }

                            Layout::item_space(ui);

                            // View Mode selection
                            ui.heading(TextStyle::heading("View Mode", TextSize::SM));
                            Layout::small_space(ui);
                            ui.horizontal(|ui| {
                                if ui.radio_value(
                                    &mut *view_mode,
                                    ViewMode::Standard,
                                    "Standard"
                                ).clicked() {
                                    info!("[MAIN_MENU] View mode set to Standard");
                                }
                                ui.add_space(5.0);
                                if ui.radio_value(
                                    &mut *view_mode,
                                    ViewMode::TempleOS,
                                    "TempleOS"
                                ).clicked() {
                                    info!("[MAIN_MENU] View mode set to TempleOS");
                                }
                            });

                            Layout::item_space(ui);

                            // AI Difficulty selection
                            ui.heading(TextStyle::heading("AI Difficulty", TextSize::SM));
                            Layout::small_space(ui);
                            ui.horizontal(|ui| {
                                ui.radio_value(&mut ai_config.difficulty, AIDifficulty::Easy, "Easy");
                                ui.radio_value(&mut ai_config.difficulty, AIDifficulty::Medium, "Med");
                                ui.radio_value(&mut ai_config.difficulty, AIDifficulty::Hard, "Hard");
                            });
                        });
                    });

                    Layout::section_space(ui);

                    // === OPTIONS SECTION ===
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;

                        if StyledButton::secondary(ui, "Settings").clicked() {
                            previous_state.state = GameState::MainMenu;
                            next_state.set(GameState::Settings);
                            menu_expanded.expanded = false;
                        }

                        if StyledButton::secondary(ui, "Piece Viewer").clicked() {
                            menu_state.set(crate::core::MenuState::PieceViewer);
                            menu_expanded.expanded = false;
                        }

                        if StyledButton::secondary(ui, "Stats").clicked() {
                            info!("[MAIN_MENU] Statistics button clicked (not implemented)");
                        }

                        if StyledButton::danger(ui, "Exit").clicked() {
                            info!("[MAIN_MENU] Exit button clicked");
                            std::process::exit(0);
                        }
                    });

                    Layout::small_space(ui);

                    // === VERSION INFO ===
                    ui.label(TextStyle::caption("v0.1.0 - Bevy 0.17.3"));
                });
            }
        });

    Ok(())
}
