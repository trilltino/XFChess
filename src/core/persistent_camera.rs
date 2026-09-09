use bevy::prelude::*;
use bevy_egui::PrimaryEguiContext;

#[derive(Resource, Default)]
pub struct PersistentEguiCamera {
    pub entity: Option<Entity>,
}

pub fn setup_persistent_egui_camera(
    mut commands: Commands,
    mut persistent_camera: ResMut<PersistentEguiCamera>,
) {
    debug!(
        "[PRESTARTUP] DEBUG: Current persistent_camera.entity: {:?}",
        persistent_camera.entity
    );

    info!("[PERSISTENT_CAMERA] Spawning persistent egui camera during PreStartup...");
    let camera_entity = commands
        .spawn((
            Camera3d::default(),
            PrimaryEguiContext,
            // Default position - will be updated by each state
            Transform::from_xyz(0.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
            Name::new("Persistent Egui camera"),
        ))
        .id();

    persistent_camera.entity = Some(camera_entity);
    debug!(
        "[PRESTARTUP] Persistent Egui camera created with entity ID: {:?}",
        camera_entity
    );
    debug!(
        "[PRESTARTUP] DEBUG: Updated persistent_camera.entity to: {:?}",
        persistent_camera.entity
    );

    // Verify the entity was created successfully
    if persistent_camera.entity.is_some() {
    } else {
        error!("[PRESTARTUP] ERROR: Failed to store camera entity in resource!");
    }
}
