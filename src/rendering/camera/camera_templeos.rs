//! TempleOS camera setup
//!
//! Provides an isometric orthographic camera view for the TempleOS chess board mode.
//! The camera uses orthographic projection to create a true 2D isometric view.

use crate::core::GameState;
use crate::game::view_mode::ViewMode;
use bevy::camera::ScalingMode;
use bevy::prelude::*;

/// Marker component for TempleOS camera
#[derive(Component)]
pub struct TempleOSCamera;

/// Component to store the initial look-at offset for maintaining isometric angle
#[derive(Component)]
pub struct TempleOSCameraLookAt {
    pub offset: Vec3,
}

/// Setup TempleOS camera with isometric orthographic view
///
/// Uses orthographic projection to eliminate perspective distortion and create
/// a true 2D isometric view. The camera is positioned at an isometric angle
/// looking at the center of the board (3.5, 0.0, 3.5).
pub fn setup_templeos_camera(mut commands: Commands, view_mode: Res<ViewMode>) {
    // Only setup TempleOS camera if in TempleOS mode
    if *view_mode != ViewMode::TempleOS {
        return;
    }

    // Board center is at (3.5, 0.0, 3.5) - middle of 8x8 board
    let board_center = Vec3::new(3.5, 0.0, 3.5);

    // Position camera at isometric angle matching Bevy orthographic example
    // Use equal distances on all axes (like the example's 5.0, 5.0, 5.0)
    // This creates a true isometric view, not a bird's eye view
    // Offset from board center to match the example's angle
    let offset = 5.0;
    let camera_position = Vec3::new(
        board_center.x + offset,
        offset,
        board_center.z + offset,
    );

    // Calculate initial look-at offset
    let look_at_offset = board_center - camera_position;

    commands.spawn((
        Camera3d::default(),
        Projection::from(OrthographicProjection {
            // Set viewport height to show the entire 8x8 board
            // Adjust this value to zoom in/out
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: 12.0,
            },
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_translation(camera_position).looking_at(board_center, Vec3::Y),
        TempleOSCamera,
        TempleOSCameraLookAt {
            offset: look_at_offset,
        },
        DespawnOnExit(GameState::InGame),
        Name::new("TempleOS Camera"),
    ));

    info!(
        "[TEMPLEOS_CAMERA] TempleOS orthographic isometric camera setup complete at {:?} looking at {:?}",
        camera_position, board_center
    );
}

/// System to handle WASD camera movement for TempleOS mode
///
/// Allows panning the camera around the board using WASD keys.
/// Movement is smooth and maintains the isometric viewing angle.
pub fn templeos_camera_movement_system(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    view_mode: Res<ViewMode>,
    mut query: Query<(&mut Transform, &TempleOSCameraLookAt), (With<TempleOSCamera>, With<Camera3d>)>,
) {
    // Only move camera in TempleOS mode
    if *view_mode != ViewMode::TempleOS {
        return;
    }

    for (mut transform, look_at) in query.iter_mut() {
        // Movement speed
        let move_speed = 5.0;
        
        // Calculate movement direction
        let mut direction = Vec3::ZERO;
        
        // Get camera's forward and right vectors
        let forward = transform.forward();
        let right = transform.right();
        
        // Project onto XZ plane (maintain isometric angle)
        let forward_xz = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
        let right_xz = Vec3::new(right.x, 0.0, right.z).normalize_or_zero();
        
        // WASD movement
        if keyboard.pressed(KeyCode::KeyW) {
            direction += forward_xz;
        }
        if keyboard.pressed(KeyCode::KeyS) {
            direction -= forward_xz;
        }
        if keyboard.pressed(KeyCode::KeyD) {
            direction += right_xz;
        }
        if keyboard.pressed(KeyCode::KeyA) {
            direction -= right_xz;
        }
        
        // Normalize to prevent faster diagonal movement
        direction = direction.normalize_or_zero();
        
        // Apply movement
        let movement = direction * move_speed * time.delta_secs();
        transform.translation += movement;
        
        // Maintain isometric viewing angle by updating look-at target
        // Use the stored offset to maintain the same relative viewing angle
        let look_at_point = transform.translation + look_at.offset;
        *transform = transform.looking_at(look_at_point, Vec3::Y);
    }
}
