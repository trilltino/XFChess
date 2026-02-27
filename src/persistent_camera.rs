use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct PersistentEguiCamera {
    pub entity: Option<Entity>,
}

// use bevy_egui::{EguiContext, EguiMultipassSchedule, EguiPrimaryContextPass, PrimaryEguiContext};

/// Setup a persistent camera with Egui context that survives all state transitions
///
/// This camera is used by all UI states (MainMenu, Settings, Pause, GameOver)
/// to avoid conflicts from multiple PrimaryEguiContext cameras.
pub fn setup_persistent_egui_camera(
    mut commands: Commands,
    mut persistent_camera: ResMut<PersistentEguiCamera>,
) {
    info!(
        "[PRESTARTUP] DEBUG: Current persistent_camera.entity: {:?}",
        persistent_camera.entity
    );

    let camera_entity = commands
        .spawn((
            Camera3d::default(),
            // Default position - will be updated by each state
            Transform::from_xyz(0.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
            Name::new("Persistent Egui Camera"),
        ))
        .id();

    persistent_camera.entity = Some(camera_entity);
    info!(
        "[PRESTARTUP] Persistent Egui camera created with entity ID: {:?}",
        camera_entity
    );
    info!(
        "[PRESTARTUP] DEBUG: Updated persistent_camera.entity to: {:?}",
        persistent_camera.entity
    );

    // Verify the entity was created successfully
    if persistent_camera.entity.is_some() {
    } else {
        error!("[PRESTARTUP] ERROR: Failed to store camera entity in resource!");
    }
}
