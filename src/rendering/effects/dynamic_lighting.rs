//! Dynamic orbital lighting system for chess board
//!
//! Creates configurable point lights that orbit around the board center,
//! providing dynamic lighting effects during gameplay.
//!
//! # Features
//!
//! - Configurable number of lights (2-6)
//! - Custom colors for each light
//! - Orbital movement around board center
//! - Configurable radius, speed, and height
//! - Shadow casting support
//!
//! # Reference
//!
//! Based on patterns from `reference/bevy/examples/3d/lighting.rs`

use crate::core::{GameSettings, GameState};
use bevy::prelude::*;

/// Plugin for dynamic orbital lighting
pub struct DynamicLightingPlugin;

impl Plugin for DynamicLightingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::InGame), spawn_orbital_lights)
            .add_systems(OnExit(GameState::InGame), despawn_orbital_lights)
            .add_systems(
                Update,
                (
                    update_orbital_lights,
                    sync_light_count.after(update_orbital_lights),
                )
                    .run_if(in_state(GameState::InGame)),
            );
    }
}

/// Component marking lights that should orbit around the board
#[derive(Component, Debug)]
struct OrbitalLight {
    /// Index of this light (0 to light_count-1)
    index: usize,
}

/// Spawn orbital lights when entering InGame state
fn spawn_orbital_lights(mut commands: Commands, settings: Res<GameSettings>) {
    if !settings.dynamic_lighting.enabled {
        return;
    }

    let light_count = settings.dynamic_lighting.light_count.clamp(2, 6) as usize;
    let radius = settings.dynamic_lighting.orbital_radius;
    let height = settings.dynamic_lighting.orbital_height;
    let shadows_enabled = settings.dynamic_lighting.shadows_enabled;

    info!("[DYNAMIC_LIGHTING] Spawning {} orbital lights", light_count);

    for i in 0..light_count {
        // Calculate initial angular position
        let angle_offset = (2.0 * std::f32::consts::PI * i as f32) / light_count as f32;

        // Initial position in circle
        let x = radius * angle_offset.cos();
        let z = radius * angle_offset.sin();

        // Get color for this light
        let color = settings.dynamic_lighting.get_color(i);

        commands.spawn((
            PointLight {
                intensity: 500_000.0,
                color,
                shadows_enabled,
                range: 30.0,
                radius: 0.5,
                ..default()
            },
            Transform::from_xyz(x, height, z),
            OrbitalLight { index: i },
            DespawnOnExit(GameState::InGame),
            Name::new(format!("Orbital Light {}", i)),
        ));
    }
}

/// Update orbital light positions and colors each frame
fn update_orbital_lights(
    time: Res<Time>,
    settings: Res<GameSettings>,
    mut query: Query<(&mut Transform, &mut PointLight, &OrbitalLight)>,
) {
    if !settings.dynamic_lighting.enabled {
        return;
    }

    let radius = settings.dynamic_lighting.orbital_radius;
    let height = settings.dynamic_lighting.orbital_height;
    let speed = settings.dynamic_lighting.orbital_speed;
    let light_count = settings.dynamic_lighting.light_count.clamp(2, 6) as usize;
    let shadows_enabled = settings.dynamic_lighting.shadows_enabled;

    // Calculate current angle based on time
    let base_angle = time.elapsed_secs() * speed;

    for (mut transform, mut point_light, orbital_light) in query.iter_mut() {
        // Calculate angular offset for this light
        let angle_offset =
            (2.0 * std::f32::consts::PI * orbital_light.index as f32) / light_count as f32;

        // Calculate current angle with offset
        let angle = base_angle + angle_offset;

        // Update position in circular orbit
        transform.translation.x = radius * angle.cos();
        transform.translation.z = radius * angle.sin();
        transform.translation.y = height;

        // Update light color from settings (allows real-time color changes)
        let color = settings.dynamic_lighting.get_color(orbital_light.index);
        point_light.color = color;

        // Update shadows setting (allows real-time shadow toggling)
        point_light.shadows_enabled = shadows_enabled;
    }
}

/// Sync light count when settings change
/// Despawns excess lights or spawns new ones as needed
fn sync_light_count(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<GameSettings>,
    query: Query<(Entity, &OrbitalLight)>,
) {
    if !settings.dynamic_lighting.enabled {
        return;
    }

    let target_count = settings.dynamic_lighting.light_count.clamp(2, 6) as usize;
    let current_count = query.iter().count();

    if current_count == target_count {
        return; // Already correct
    }

    if current_count < target_count {
        // Need to spawn more lights
        let radius = settings.dynamic_lighting.orbital_radius;
        let height = settings.dynamic_lighting.orbital_height;
        let shadows_enabled = settings.dynamic_lighting.shadows_enabled;
        let speed = settings.dynamic_lighting.orbital_speed;
        let base_angle = time.elapsed_secs() * speed;

        for i in current_count..target_count {
            let angle_offset = (2.0 * std::f32::consts::PI * i as f32) / target_count as f32;
            let angle = base_angle + angle_offset;

            let x = radius * angle.cos();
            let z = radius * angle.sin();
            let color = settings.dynamic_lighting.get_color(i);

            commands.spawn((
                PointLight {
                    intensity: 500_000.0,
                    color,
                    shadows_enabled,
                    range: 30.0,
                    radius: 0.5,
                    ..default()
                },
                Transform::from_xyz(x, height, z),
                OrbitalLight { index: i },
                DespawnOnExit(GameState::InGame),
                Name::new(format!("Orbital Light {}", i)),
            ));
        }
    } else {
        // Need to despawn excess lights
        let mut lights: Vec<(Entity, usize)> = query.iter().map(|(e, ol)| (e, ol.index)).collect();
        lights.sort_by_key(|(_, idx)| *idx);

        // Despawn lights with highest indices first
        for (entity, _) in lights.iter().rev().take(current_count - target_count) {
            commands.entity(*entity).despawn();
        }
    }
}

/// Despawn all orbital lights when exiting InGame state
fn despawn_orbital_lights(mut commands: Commands, query: Query<Entity, With<OrbitalLight>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
    info!("[DYNAMIC_LIGHTING] Despawned all orbital lights");
}
