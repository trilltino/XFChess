//! Settings menu plugin
//!
//! Allows players to configure:
//! - AI difficulty
//! - Graphics quality
//! - Board theme
//! - Game preferences

use crate::core::{BoardTheme, GameSettings, GameState, GraphicsQuality, PreviousState};
use crate::ui::styles::*;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

/// Plugin for settings menu state
pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Settings), setup_settings_camera)
            .add_systems(
                EguiPrimaryContextPass,
                settings_ui_wrapper.run_if(in_state(GameState::Settings)),
            )
            .add_systems(
                Update,
                handle_settings_escape.run_if(in_state(GameState::Settings)),
            );
    }
}

/// Wrapper for settings_ui that handles Result
fn settings_ui_wrapper(
    contexts: EguiContexts,
    next_state: ResMut<NextState<GameState>>,
    previous_state: Res<PreviousState>,
    settings: ResMut<GameSettings>,
) {
    info!("[SETTINGS] UI wrapper called!");
    if let Err(e) = settings_ui(contexts, next_state, previous_state, settings) {
        error!("[SETTINGS] UI rendering failed: {:?}", e);
    } else {
        info!("[SETTINGS] UI rendered successfully!");
    }
}

/// Marker component for settings camera
#[derive(Component)]
struct SettingsCamera;

/// Setup camera for settings screen
/// Uses the persistent Egui camera and updates its transform
/// Handles case where camera might not exist yet
fn setup_settings_camera(
    persistent_camera: Res<crate::PersistentEguiCamera>,
    mut camera_query: Query<
        &mut Transform,
        (With<bevy_egui::PrimaryEguiContext>, Without<SettingsCamera>),
    >,
    mut commands: Commands,
) {
    info!("[SETTINGS] Setting up settings camera");
    info!(
        "[SETTINGS] DEBUG: Persistent camera entity: {:?}",
        persistent_camera.entity
    );

    // Update persistent camera transform for settings view
    if let Some(camera_entity) = persistent_camera.entity {
        info!(
            "[SETTINGS] DEBUG: Attempting to query camera entity {:?}",
            camera_entity
        );
        match camera_query.get_mut(camera_entity) {
            Ok(mut transform) => {
                info!("[SETTINGS] DEBUG: Successfully queried camera transform");
                *transform = Transform::from_xyz(0.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y);
                info!("[SETTINGS] Updated persistent camera transform for settings");

                // Add settings marker to persistent camera
                info!("[SETTINGS] DEBUG: Adding SettingsCamera component");
                commands.entity(camera_entity).insert(SettingsCamera);
                info!("[SETTINGS] DEBUG: SettingsCamera component added successfully");
            }
            Err(e) => {
                error!(
                    "[SETTINGS] ERROR: Persistent camera entity {:?} exists but query failed: {:?}",
                    camera_entity, e
                );
                error!("[SETTINGS] ERROR: Query filter: With<PrimaryEguiContext>, Without<SettingsCamera>");
            }
        }
    } else {
        error!("[SETTINGS] ERROR: Persistent camera not yet created. Entity: None");
        error!("[SETTINGS] ERROR: This should not happen for Settings state (not default state)");
    }

    info!("[SETTINGS] Camera setup complete");
}

/// Settings menu UI
fn settings_ui(
    mut contexts: EguiContexts,
    mut next_state: ResMut<NextState<GameState>>,
    previous_state: Res<PreviousState>,
    mut settings: ResMut<GameSettings>,
) -> Result<(), bevy::ecs::query::QuerySingleError> {
    let ctx = contexts.ctx_mut()?;

    egui::CentralPanel::default()
        .frame(egui::Frame {
            fill: UiColors::BG_DARK,
            ..Default::default()
        })
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                Layout::section_space(ui);

                // Debug text to confirm UI is rendering
                ui.colored_label(
                    egui::Color32::from_rgb(255, 255, 0),
                    egui::RichText::new("SETTINGS - UI IS WORKING!").size(24.0),
                );

                Layout::small_space(ui);

                ui.heading(TextStyle::heading("Settings", TextSize::LG));

                Layout::section_space(ui);

                // Graphics Quality
                StyledPanel::card().show(ui, |ui| {
                    ui.heading(TextStyle::heading("Graphics Quality", TextSize::MD));
                    Layout::item_space(ui);

                    ui.horizontal(|ui| {
                        ui.radio_value(&mut settings.graphics_quality, GraphicsQuality::Low, "Low");
                        ui.radio_value(
                            &mut settings.graphics_quality,
                            GraphicsQuality::Medium,
                            "Medium",
                        );
                        ui.radio_value(
                            &mut settings.graphics_quality,
                            GraphicsQuality::High,
                            "High",
                        );
                        ui.radio_value(
                            &mut settings.graphics_quality,
                            GraphicsQuality::Ultra,
                            "Ultra",
                        );
                    });

                    Layout::small_space(ui);
                    ui.label(TextStyle::caption(settings.graphics_quality.description()));
                });

                Layout::item_space(ui);

                // Board Theme
                StyledPanel::card().show(ui, |ui| {
                    ui.heading(TextStyle::heading("Board Theme", TextSize::MD));
                    Layout::item_space(ui);

                    ui.horizontal(|ui| {
                        ui.radio_value(&mut settings.board_theme, BoardTheme::Classic, "Classic");
                        ui.radio_value(&mut settings.board_theme, BoardTheme::Modern, "Modern");
                        ui.radio_value(&mut settings.board_theme, BoardTheme::Wood, "Wood");
                        ui.radio_value(&mut settings.board_theme, BoardTheme::Marble, "Marble");
                    });
                });

                Layout::item_space(ui);

                // Dynamic Lighting Configuration
                StyledPanel::card().show(ui, |ui| {
                    ui.heading(TextStyle::heading("Dynamic Lighting", TextSize::MD));
                    Layout::item_space(ui);

                    // Enable/Disable toggle
                    ui.checkbox(
                        &mut settings.dynamic_lighting.enabled,
                        "Enable dynamic orbital lighting",
                    );

                    if settings.dynamic_lighting.enabled {
                        Layout::item_space(ui);

                        // Light count slider
                        ui.label(TextStyle::body(format!(
                            "Number of Lights: {}",
                            settings.dynamic_lighting.light_count
                        )));
                        ui.add(egui::Slider::new(
                            &mut settings.dynamic_lighting.light_count,
                            2..=6,
                        ));

                        // Update colors when count changes
                        let old_count = settings.dynamic_lighting.light_colors.len() as u32;
                        if old_count != settings.dynamic_lighting.light_count {
                            settings.dynamic_lighting.update_colors_for_count();
                        }

                        Layout::item_space(ui);

                        // Color pickers for each light
                        ui.label(TextStyle::body("Light Colors:"));
                        for i in 0..settings.dynamic_lighting.light_count as usize {
                            if i < settings.dynamic_lighting.light_colors.len() {
                                let color = &mut settings.dynamic_lighting.light_colors[i];
                                // Convert Color to RGB values for sliders
                                let srgba = color.to_srgba();
                                let mut r = srgba.red;
                                let mut g = srgba.green;
                                let mut b = srgba.blue;

                                ui.horizontal(|ui| {
                                    ui.label(format!("Light {}: ", i + 1));
                                    ui.add(egui::Slider::new(&mut r, 0.0..=1.0).text("R"));
                                    ui.add(egui::Slider::new(&mut g, 0.0..=1.0).text("G"));
                                    ui.add(egui::Slider::new(&mut b, 0.0..=1.0).text("B"));

                                    // Update color if sliders changed
                                    if r != srgba.red || g != srgba.green || b != srgba.blue {
                                        *color = Color::srgb(r, g, b);
                                    }
                                });
                            }
                        }

                        Layout::item_space(ui);

                        // Orbital parameters
                        ui.label(TextStyle::body("Orbital Radius:"));
                        ui.add(egui::Slider::new(
                            &mut settings.dynamic_lighting.orbital_radius,
                            5.0..=20.0,
                        ));

                        ui.label(TextStyle::body("Orbital Speed:"));
                        ui.add(egui::Slider::new(
                            &mut settings.dynamic_lighting.orbital_speed,
                            0.1..=2.0,
                        ));

                        ui.label(TextStyle::body("Light Height:"));
                        ui.add(egui::Slider::new(
                            &mut settings.dynamic_lighting.orbital_height,
                            5.0..=15.0,
                        ));

                        Layout::item_space(ui);

                        // Shadows toggle
                        ui.checkbox(
                            &mut settings.dynamic_lighting.shadows_enabled,
                            "Cast shadows",
                        );
                    }
                });

                Layout::item_space(ui);

                // Game Preferences
                StyledPanel::card().show(ui, |ui| {
                    ui.heading(TextStyle::heading("Game Preferences", TextSize::MD));
                    Layout::item_space(ui);

                    ui.checkbox(&mut settings.show_hints, "Show move hints");
                    ui.checkbox(&mut settings.highlight_last_move, "Highlight last move");

                    Layout::item_space(ui);

                    ui.label(TextStyle::body("Master Volume"));
                    ui.add(egui::Slider::new(&mut settings.master_volume, 0.0..=1.0));
                });

                Layout::section_space(ui);

                // Back button
                if StyledButton::secondary(ui, "Back").clicked() {
                    next_state.set(previous_state.state);
                }
            });
        });

    Ok(())
}

/// Handle escape key to return to previous state
fn handle_settings_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
    previous_state: Res<PreviousState>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        info!(
            "[SETTINGS] Escape pressed, returning to {:?}",
            previous_state.state
        );
        next_state.set(previous_state.state);
    }
}
