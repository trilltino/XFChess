//! Graphics quality settings application
//!
//! Applies graphics quality presets to cameras and lights based on GameSettings.

use crate::core::GameSettings;
use bevy::pbr::ScreenSpaceAmbientOcclusion;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;

/// System that applies graphics quality settings to cameras
///
/// Watches for changes to `GameSettings.graphics_quality` and updates:
/// - Bloom component on cameras
/// - ScreenSpaceAmbientOcclusion component on cameras
pub fn apply_graphics_quality_camera_system(
    settings: Res<GameSettings>,
    mut camera_query: Query<
        Entity,
        (
            With<Camera3d>,
            Without<Bloom>,
            Without<ScreenSpaceAmbientOcclusion>,
        ),
    >,
    mut commands: Commands,
    mut last_quality: Local<Option<crate::core::GraphicsQuality>>,
) {
    // Check if quality changed
    let current_quality = settings.graphics_quality;
    if let Some(prev_quality) = *last_quality {
        if prev_quality == current_quality {
            return; // No change
        }
    }
    *last_quality = Some(current_quality);

    let bloom_enabled = settings.graphics_quality.bloom_enabled();
    let ssao_enabled = settings.graphics_quality.ambient_occlusion_enabled();

    // Apply settings to all cameras
    for entity in camera_query.iter_mut() {
        if bloom_enabled {
            commands.entity(entity).insert(Bloom::NATURAL);
        }
        if ssao_enabled {
            commands
                .entity(entity)
                .insert(ScreenSpaceAmbientOcclusion::default());
        }
    }

    info!(
        "[GRAPHICS] Applied quality preset: {:?} (Bloom: {}, SSAO: {})",
        settings.graphics_quality.description(),
        bloom_enabled,
        ssao_enabled
    );
}

/// System that updates graphics quality settings on existing cameras
///
/// Updates cameras that already have Bloom or SSAO components when quality changes.
pub fn update_graphics_quality_camera_system(
    settings: Res<GameSettings>,
    mut bloom_query: Query<Entity, (With<Camera3d>, With<Bloom>)>,
    mut ssao_query: Query<Entity, (With<Camera3d>, With<ScreenSpaceAmbientOcclusion>)>,
    mut commands: Commands,
    mut last_quality: Local<Option<crate::core::GraphicsQuality>>,
) {
    // Check if quality changed
    let current_quality = settings.graphics_quality;
    if let Some(prev_quality) = *last_quality {
        if prev_quality == current_quality {
            return; // No change
        }
    }
    *last_quality = Some(current_quality);

    let bloom_enabled = settings.graphics_quality.bloom_enabled();
    let ssao_enabled = settings.graphics_quality.ambient_occlusion_enabled();

    // Remove or add Bloom based on quality
    if !bloom_enabled {
        for entity in bloom_query.iter_mut() {
            commands.entity(entity).remove::<Bloom>();
        }
    } else {
        // Add to cameras that don't have it (handled by apply system)
    }

    // Remove or add SSAO based on quality
    if !ssao_enabled {
        for entity in ssao_query.iter_mut() {
            commands
                .entity(entity)
                .remove::<ScreenSpaceAmbientOcclusion>();
        }
    }

    info!(
        "[GRAPHICS] Updated quality preset: {:?} (Bloom: {}, SSAO: {})",
        settings.graphics_quality.description(),
        bloom_enabled,
        ssao_enabled
    );
}

/// System that applies shadow settings to lights based on graphics quality
///
/// Updates all lights' shadows_enabled property based on graphics quality preset.
pub fn apply_graphics_quality_lights_system(
    settings: Res<GameSettings>,
    mut directional_lights: Query<&mut DirectionalLight>,
    mut point_lights: Query<&mut PointLight>,
    mut spot_lights: Query<&mut SpotLight>,
    mut last_quality: Local<Option<crate::core::GraphicsQuality>>,
) {
    // Check if quality changed
    let current_quality = settings.graphics_quality;
    if let Some(prev_quality) = *last_quality {
        if prev_quality == current_quality {
            return; // No change
        }
    }
    *last_quality = Some(current_quality);

    let shadows_enabled = settings.graphics_quality.shadow_enabled();

    // Update all light types
    for mut light in directional_lights.iter_mut() {
        light.shadows_enabled = shadows_enabled;
    }
    for mut light in point_lights.iter_mut() {
        light.shadows_enabled = shadows_enabled;
    }
    for mut light in spot_lights.iter_mut() {
        light.shadows_enabled = shadows_enabled;
    }

    info!("[GRAPHICS] Updated light shadows: {}", shadows_enabled);
}
