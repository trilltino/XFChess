use crate::core::{game_scene, DynamicLightingSettings, GameSettings, GameState};
use bevy::prelude::*;

pub struct DynamicLightingPlugin;

impl Plugin for DynamicLightingPlugin {
    fn build(&self, app: &mut App) {
        // The lights carry game_scene(), so they go with the rest of the scene.
        app.add_systems(OnEnter(GameState::InGame), spawn_orbital_lights)
            .add_systems(
                Update,
                (update_orbital_lights, sync_light_count)
                    .chain()
                    .after(crate::game::systems::animate_capture_fade)
                    .after(crate::rendering::effects::update_check_highlight_system)
                    .before(crate::game::systems::visual::update_board_fill_light)
                    .in_set(crate::core::schedule::GameSystems::Visual),
            );
    }
}

#[derive(Component, Debug)]
struct OrbitalLight {
    index: usize,
}

fn spawn_orbital_lights(mut commands: Commands, settings: Res<GameSettings>) {
    if !settings.dynamic_lighting.enabled {
        return;
    }

    let quality_cap = DynamicLightingSettings::quality_cap(settings.graphics_quality);
    let light_count = settings
        .dynamic_lighting
        .light_count
        .min(quality_cap)
        .clamp(1, 6) as usize;
    let radius = settings.dynamic_lighting.orbital_radius;
    let height = settings.dynamic_lighting.orbital_height;
    let shadows_enabled = settings.dynamic_lighting.shadows_enabled;

    info!(
        "[DYNAMIC_LIGHTING] Spawning {} orbital lights (quality cap: {})",
        light_count, quality_cap
    );

    for i in 0..light_count {
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
                shadow_maps_enabled: shadows_enabled,
                range: 30.0,
                radius: 0.5,
                ..default()
            },
            Transform::from_xyz(x, height, z),
            OrbitalLight { index: i },
            game_scene(),
            Name::new(format!("Orbital Light {}", i)),
        ));
    }
}

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

    let base_angle = time.elapsed_secs() * speed;

    for (mut transform, mut point_light, orbital_light) in query.iter_mut() {
        let angle_offset =
            (2.0 * std::f32::consts::PI * orbital_light.index as f32) / light_count as f32;

        let angle = base_angle + angle_offset;

        transform.translation.x = radius * angle.cos();
        transform.translation.z = radius * angle.sin();
        transform.translation.y = height;

        let color = settings.dynamic_lighting.get_color(orbital_light.index);
        point_light.color = color;

        point_light.shadow_maps_enabled = shadows_enabled;
    }
}

fn sync_light_count(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<GameSettings>,
    query: Query<(Entity, &OrbitalLight)>,
) {
    if !settings.dynamic_lighting.enabled {
        return;
    }

    let quality_cap = DynamicLightingSettings::quality_cap(settings.graphics_quality);
    let target_count = settings
        .dynamic_lighting
        .light_count
        .min(quality_cap)
        .clamp(1, 6) as usize;
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
                    shadow_maps_enabled: shadows_enabled,
                    range: 30.0,
                    radius: 0.5,
                    ..default()
                },
                Transform::from_xyz(x, height, z),
                OrbitalLight { index: i },
                game_scene(),
                bevy::camera::visibility::RenderLayers::layer(
                    crate::game::systems::camera::BOARD_LAYER,
                ),
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
