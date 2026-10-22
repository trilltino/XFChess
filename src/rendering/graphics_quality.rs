//! Applies the graphics-quality setting to cameras (bloom, SSAO) and lights
//! (shadows). Runs when the setting changes and whenever a camera or light is
//! spawned, so the game scene built after the menu gets the same treatment.

use bevy::core_pipeline::prepass::{DepthPrepass, NormalPrepass};
use bevy::pbr::ScreenSpaceAmbientOcclusion;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::Msaa;

use crate::core::GameSettings;

/// Runs the quality systems in `ApplySettings`, only on frames where the setting
/// changed or a camera or light was spawned.
pub(crate) fn register(app: &mut App) {
    app.add_systems(
        Update,
        (
            apply_camera_quality.run_if(settings_changed_or_added::<Camera3d>),
            apply_light_quality.run_if(
                settings_changed_or_added::<PointLight>
                    .or_else(settings_changed_or_added::<DirectionalLight>)
                    .or_else(settings_changed_or_added::<SpotLight>),
            ),
        )
            .in_set(crate::core::schedule::ApplySettings),
    );
}

/// Whether `GameSettings` changed this frame or `T` was just spawned.
pub fn settings_changed_or_added<T: Component>(
    settings: Option<Res<GameSettings>>,
    added: Query<(), Added<T>>,
) -> bool {
    settings.is_some_and(|s| s.is_changed()) || !added.is_empty()
}

/// Adds or removes bloom and SSAO on 3D cameras. A camera's own bloom tuning is
/// kept when bloom stays on. SSAO only renders with MSAA off (Bevy logs an error
/// every frame otherwise), so MSAA is switched off with it and restored after.
pub fn apply_camera_quality(
    settings: Res<GameSettings>,
    cameras: Query<(
        Entity,
        Ref<Camera3d>,
        Has<Bloom>,
        Has<ScreenSpaceAmbientOcclusion>,
    )>,
    mut commands: Commands,
) {
    let quality = settings.graphics_quality;
    for (entity, camera, has_bloom, has_ssao) in &cameras {
        if !(settings.is_changed() || camera.is_added()) {
            continue;
        }
        let mut camera = commands.entity(entity);
        match (quality.bloom_enabled(), has_bloom) {
            (true, false) => {
                camera.insert(Bloom::NATURAL);
            }
            (false, true) => {
                camera.remove::<Bloom>();
            }
            _ => {}
        }
        match (quality.ambient_occlusion_enabled(), has_ssao) {
            (true, false) => {
                camera.insert((ScreenSpaceAmbientOcclusion::default(), Msaa::Off));
            }
            (false, true) => {
                // The prepasses were added for SSAO; nothing else here uses them.
                camera
                    .remove::<(ScreenSpaceAmbientOcclusion, DepthPrepass, NormalPrepass)>()
                    .insert(Msaa::default());
            }
            _ => {}
        }
    }
}

/// The shadow setting a light was spawned with; quality can switch shadows off
/// but never turns them on for a light designed without them.
#[derive(Component, Clone, Copy)]
pub struct DesignedShadows(pub bool);

/// Caps light shadows to the quality preset.
#[allow(clippy::type_complexity)]
pub fn apply_light_quality(
    settings: Res<GameSettings>,
    mut point: Query<(Entity, Mut<PointLight>, Option<&DesignedShadows>)>,
    mut directional: Query<(Entity, Mut<DirectionalLight>, Option<&DesignedShadows>)>,
    mut spot: Query<(Entity, Mut<SpotLight>, Option<&DesignedShadows>)>,
    mut commands: Commands,
) {
    let quality = settings.graphics_quality;
    let allowed = quality.shadow_enabled();
    // Smaller shadow maps need more bias to hide acne.
    let bias = 4096.0 / quality.shadow_map_size() as f32 * 0.05;
    let changed = settings.is_changed();

    // Records the designed value the first time a light is seen; returns it.
    let mut designed = |entity: Entity, current: bool, known: Option<&DesignedShadows>| {
        known.map_or_else(
            || {
                commands.entity(entity).insert(DesignedShadows(current));
                current
            },
            |d| d.0,
        )
    };

    for (entity, mut light, known) in &mut point {
        if changed || light.is_added() {
            let shadows = designed(entity, light.shadow_maps_enabled, known) && allowed;
            light.shadow_maps_enabled = shadows;
            light.shadow_depth_bias = bias;
        }
    }
    for (entity, mut light, known) in &mut directional {
        if changed || light.is_added() {
            light.shadow_maps_enabled =
                designed(entity, light.shadow_maps_enabled, known) && allowed;
        }
    }
    for (entity, mut light, known) in &mut spot {
        if changed || light.is_added() {
            light.shadow_maps_enabled =
                designed(entity, light.shadow_maps_enabled, known) && allowed;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::GraphicsQuality;

    fn app(quality: GraphicsQuality) -> App {
        let mut app = App::new();
        app.insert_resource(GameSettings {
            graphics_quality: quality,
            ..Default::default()
        });
        register(&mut app);
        app.update();
        app
    }

    fn set_quality(app: &mut App, quality: GraphicsQuality) {
        app.world_mut()
            .resource_mut::<GameSettings>()
            .graphics_quality = quality;
        app.update();
    }

    #[test]
    fn a_camera_spawned_later_gets_the_current_quality() {
        let mut app = app(GraphicsQuality::Ultra);
        // Frames pass with no camera, as on the menu before a game starts.
        app.update();
        let camera = app.world_mut().spawn(Camera3d::default()).id();
        app.update();

        let world = app.world();
        assert!(world.get::<Bloom>(camera).is_some(), "bloom on Ultra");
        assert!(world.get::<ScreenSpaceAmbientOcclusion>(camera).is_some());
        assert_eq!(
            world.get::<Msaa>(camera),
            Some(&Msaa::Off),
            "SSAO needs MSAA off"
        );
    }

    #[test]
    fn lowering_quality_removes_effects_and_restores_msaa() {
        let mut app = app(GraphicsQuality::Ultra);
        let camera = app.world_mut().spawn(Camera3d::default()).id();
        app.update();

        set_quality(&mut app, GraphicsQuality::Medium);
        let world = app.world();
        assert!(world.get::<Bloom>(camera).is_none());
        assert!(world.get::<ScreenSpaceAmbientOcclusion>(camera).is_none());
        assert!(world.get::<DepthPrepass>(camera).is_none());
        assert_eq!(world.get::<Msaa>(camera), Some(&Msaa::default()));
    }

    #[test]
    fn quality_caps_light_shadows_but_never_adds_them() -> Result<(), Box<dyn std::error::Error>> {
        let mut app = app(GraphicsQuality::Low);
        let key = app
            .world_mut()
            .spawn(PointLight {
                shadow_maps_enabled: true,
                ..Default::default()
            })
            .id();
        let fill = app
            .world_mut()
            .spawn(PointLight {
                shadow_maps_enabled: false,
                ..Default::default()
            })
            .id();
        app.update();
        let shadows = |app: &App, light| {
            app.world()
                .get::<PointLight>(light)
                .map(|light| light.shadow_maps_enabled)
        };
        assert_eq!(shadows(&app, key), Some(false), "Low turns shadows off");

        set_quality(&mut app, GraphicsQuality::High);
        assert_eq!(
            shadows(&app, key),
            Some(true),
            "the key light gets its shadows back"
        );
        assert_eq!(
            shadows(&app, fill),
            Some(false),
            "a light designed without shadows stays without"
        );

        Ok(())
    }

    #[test]
    fn nothing_runs_without_a_change() {
        let mut app = app(GraphicsQuality::Ultra);
        let camera = app.world_mut().spawn(Camera3d::default()).id();
        app.update();
        // A camera that dropped its bloom on purpose keeps it off until the
        // setting changes again.
        app.world_mut().entity_mut(camera).remove::<Bloom>();
        app.update();
        assert!(app.world().get::<Bloom>(camera).is_none());
    }
}
