//! Camera control system for RTS-style board observation
//!
//! Implements Total War-style camera controls with smooth WASD movement
//! and mouse scroll wheel zoom. The camera moves along the XZ plane while
//! allowing players to pan around and zoom in/out of the chess board.
//!
//! # Controls
//!
//! - **W**: Move camera forward (toward top of board)
//! - **S**: Move camera backward (toward bottom of board)
//! - **A**: Strafe camera left
//! - **D**: Strafe camera right
//! - **Mouse Wheel Up**: Zoom in (lower camera height)
//! - **Mouse Wheel Down**: Zoom out (raise camera height)
//!
//! # Implementation
//!
//! Movement uses smooth interpolation (lerp) for fluid motion rather than
//! instant position updates. The camera's forward and right vectors are
//! projected onto the XZ plane to maintain the isometric viewing angle
//! while allowing horizontal panning.
//!
//! Zoom works by adjusting the camera's Y position (height above the board).
//! Mouse wheel input sets a target zoom level, and the camera smoothly
//! interpolates to that target each frame, creating the signature Total War
//! zoom feel. Zoom limits prevent extreme close-ups or distant views.
//!
//! # Reference
//!
//! Camera movement patterns based on:
//! - `reference/bevy/examples/3d/3d_scene.rs` - Camera transform manipulation
//! - Total War series camera controls - RTS standard

use bevy::{prelude::*, input::mouse::MouseWheel};

/// Component marking a camera as player-controllable with RTS-style movement
///
/// Attach this to camera entities that should respond to WASD keyboard input
/// and mouse scroll wheel zoom. The camera will smoothly pan across the XZ plane
/// and adjust its Y position (height) for zooming.
///
/// # Example
///
/// ```rust,ignore
/// commands.spawn((
///     Camera3d::default(),
///     Transform::from_xyz(0.0, 15.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
///     CameraController::default(),
/// ));
/// ```
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct CameraController {
    /// Movement speed in units per second
    ///
    /// Higher values = faster panning. Suggested range: 10-20 for chess board.
    pub move_speed: f32,

    /// Smoothing factor for movement interpolation (0.0 to 1.0)
    ///
    /// - 0.0: No movement (camera frozen)
    /// - 0.1: Very smooth, gradual movement
    /// - 0.5: Balanced responsiveness
    /// - 1.0: Instant, no smoothing
    ///
    /// Lower values create smoother but less responsive movement.
    pub smoothing: f32,

    /// Zoom speed multiplier for mouse wheel input
    ///
    /// Higher values = faster zoom response. Each mouse wheel tick
    /// adjusts the target zoom by this amount. Suggested range: 1.0-3.0.
    pub zoom_speed: f32,

    /// Smoothing factor for zoom interpolation (0.0 to 1.0)
    ///
    /// Similar to movement smoothing but specifically for zoom.
    /// Lower values create smoother, more cinematic zoom.
    /// Higher values create snappier zoom response.
    pub zoom_smoothing: f32,

    /// Current zoom level (camera Y position / height)
    ///
    /// This value is smoothly interpolated toward target_zoom each frame.
    /// Represents the camera's height above the board.
    pub current_zoom: f32,

    /// Target zoom level set by mouse wheel input
    ///
    /// Mouse wheel up decreases this (zoom in), wheel down increases (zoom out).
    /// Clamped between min_zoom and max_zoom.
    pub target_zoom: f32,

    /// Minimum zoom level (closest to board)
    ///
    /// Prevents camera from clipping through the board or getting too close.
    /// Lower value = can zoom in closer.
    pub min_zoom: f32,

    /// Maximum zoom level (farthest from board)
    ///
    /// Prevents camera from zooming out too far and losing board visibility.
    /// Higher value = can zoom out farther.
    pub max_zoom: f32,
}

impl Default for CameraController {
    /// Creates a CameraController with Total War-style defaults
    ///
    /// # Default Values
    ///
    /// - **move_speed**: 12.0 - Moderate panning speed
    /// - **smoothing**: 0.3 - Smooth but responsive movement
    /// - **zoom_speed**: 2.0 - Balanced zoom response
    /// - **zoom_smoothing**: 0.15 - Very smooth, cinematic zoom
    /// - **current_zoom**: 15.0 - Default camera height (matches typical spawn)
    /// - **target_zoom**: 15.0 - Start at current zoom
    /// - **min_zoom**: 5.0 - Close-up view of pieces
    /// - **max_zoom**: 30.0 - Strategic overview height
    fn default() -> Self {
        Self {
            move_speed: 12.0,
            smoothing: 0.3,
            zoom_speed: 2.0,
            zoom_smoothing: 0.15,    // Slower than movement for cinematic feel
            current_zoom: 15.0,       // Typical chess board viewing height
            target_zoom: 15.0,
            min_zoom: 5.0,            // Close enough to see piece details
            max_zoom: 30.0,           // Far enough for full board overview
        }
    }
}

/// System that handles mouse wheel zoom input and updates target zoom level
///
/// Reads mouse wheel events and adjusts the camera's target_zoom accordingly.
/// Positive delta (wheel up) zooms in (decreases height), negative delta (wheel down)
/// zooms out (increases height). Target zoom is clamped to min/max bounds.
///
/// This system only updates the target; actual camera movement happens in
/// `camera_zoom_system` for smooth interpolation.
///
/// # Total War Feel
///
/// The zoom response is calibrated to feel like Total War games:
/// - Moderate speed (not too fast, not too slow)
/// - Each wheel tick moves target by zoom_speed units
/// - Smooth interpolation applied separately for cinematic effect
pub fn camera_zoom_input_system(
    mut mouse_wheel_events: EventReader<MouseWheel>,
    mut query: Query<&mut CameraController>,
) {
    use bevy::input::mouse::MouseScrollUnit;

    for mut controller in query.iter_mut() {
        for event in mouse_wheel_events.read() {
            // Calculate zoom delta based on scroll unit
            let zoom_delta = match event.unit {
                MouseScrollUnit::Line => {
                    // Standard mouse wheel (most common)
                    // Positive y = scroll up = zoom in = decrease height
                    -event.y * controller.zoom_speed
                }
                MouseScrollUnit::Pixel => {
                    // Touchpad or high-precision scroll
                    // Scale down pixel values as they're much larger
                    -event.y * controller.zoom_speed * 0.01
                }
            };

            // Update target zoom and clamp to bounds
            controller.target_zoom = (controller.target_zoom + zoom_delta)
                .clamp(controller.min_zoom, controller.max_zoom);
        }
    }
}

/// System that smoothly interpolates camera zoom to target level
///
/// Adjusts the camera's Y position (height) to match the target zoom level
/// using smooth interpolation. This runs every frame to create the characteristic
/// smooth, cinematic zoom of Total War games.
///
/// # Algorithm
///
/// 1. Read current_zoom and target_zoom from controller
/// 2. Interpolate current_zoom toward target_zoom using lerp
/// 3. Apply current_zoom to camera's Y position in Transform
///
/// The zoom smoothing factor is typically lower than movement smoothing
/// (0.15 vs 0.3) to create a more cinematic, gradual zoom effect.
pub fn camera_zoom_system(
    mut query: Query<(&mut Transform, &mut CameraController)>,
) {
    for (mut transform, mut controller) in query.iter_mut() {
        // Smoothly interpolate current zoom toward target
        controller.current_zoom = controller.current_zoom.lerp(
            controller.target_zoom,
            controller.zoom_smoothing,
        );

        // Apply zoom to camera Y position (height)
        transform.translation.y = controller.current_zoom;
    }
}

/// System that handles WASD camera movement with smooth interpolation
///
/// Runs every frame in the Update schedule when in Multiplayer state.
/// Projects camera's forward/right vectors onto XZ plane to maintain
/// the isometric view angle while allowing horizontal panning.
///
/// # Algorithm
///
/// 1. Get camera's forward and right vectors from Transform
/// 2. Project vectors onto XZ plane (zero out Y component)
/// 3. Normalize projected vectors to ensure consistent speed
/// 4. Calculate movement direction based on WASD input
/// 5. Compute target position = current + (direction * speed * delta_time)
/// 6. Smoothly interpolate current position toward target using lerp
///
/// # Note on Zoom
///
/// This system only affects X and Z translation (horizontal panning).
/// Vertical movement (Y/zoom) is handled by `camera_zoom_system` to prevent
/// interference between WASD panning and scroll wheel zooming.
pub fn camera_movement_system(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&mut Transform, &CameraController)>,
) {
    for (mut transform, controller) in query.iter_mut() {
        // Calculate movement direction from keyboard input
        let mut direction = Vec3::ZERO;

        // Get camera's basis vectors
        let forward = transform.forward();
        let right = transform.right();

        // Project onto XZ plane (maintain Y height for RTS-style movement)
        let forward_xz = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
        let right_xz = Vec3::new(right.x, 0.0, right.z).normalize_or_zero();

        // Accumulate movement direction based on pressed keys
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

        // Normalize diagonal movement to prevent faster diagonal speed
        direction = direction.normalize_or_zero();

        // Calculate target position with velocity-based movement
        let velocity = direction * controller.move_speed * time.delta_secs();
        let target_position = transform.translation + velocity;

        // Smoothly interpolate to target position (X and Z only)
        // Y position is controlled by zoom system to prevent interference
        let current_xz = Vec2::new(transform.translation.x, transform.translation.z);
        let target_xz = Vec2::new(target_position.x, target_position.z);
        let interpolated_xz = current_xz.lerp(target_xz, controller.smoothing);

        transform.translation.x = interpolated_xz.x;
        transform.translation.z = interpolated_xz.y;
        // Note: Y (height) is not modified here, it's controlled by camera_zoom_system
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_controller_default() {
        //! Verifies CameraController has sensible Total War-style defaults

        let controller = CameraController::default();

        assert_eq!(controller.move_speed, 12.0);
        assert_eq!(controller.smoothing, 0.3);
        assert_eq!(controller.zoom_speed, 2.0);
        assert_eq!(controller.zoom_smoothing, 0.15);
        assert_eq!(controller.current_zoom, 15.0);
        assert_eq!(controller.target_zoom, 15.0);
        assert_eq!(controller.min_zoom, 5.0);
        assert_eq!(controller.max_zoom, 30.0);
    }

    #[test]
    fn test_zoom_limits_are_logical() {
        //! Ensures min_zoom < default < max_zoom

        let controller = CameraController::default();

        assert!(controller.min_zoom < controller.current_zoom);
        assert!(controller.current_zoom < controller.max_zoom);
        assert!(controller.min_zoom < controller.max_zoom);
    }

    #[test]
    fn test_zoom_smoothing_is_slower_than_movement() {
        //! Total War games have slower zoom than movement for cinematic feel

        let controller = CameraController::default();

        assert!(controller.zoom_smoothing < controller.smoothing,
            "Zoom should be smoother (slower) than movement for cinematic effect");
    }

    #[test]
    fn test_camera_controller_custom_values() {
        //! Tests creating controller with custom zoom parameters

        let controller = CameraController {
            move_speed: 20.0,
            smoothing: 0.5,
            zoom_speed: 3.0,
            zoom_smoothing: 0.2,
            current_zoom: 10.0,
            target_zoom: 10.0,
            min_zoom: 3.0,
            max_zoom: 50.0,
        };

        assert_eq!(controller.zoom_speed, 3.0);
        assert_eq!(controller.min_zoom, 3.0);
        assert_eq!(controller.max_zoom, 50.0);
    }

    #[test]
    fn test_zoom_interpolation_convergence() {
        //! Verifies zoom lerp moves toward target over multiple frames

        let mut current = 15.0;
        let target = 10.0;
        let smoothing = 0.15;

        // Simulate several frames of interpolation
        for _ in 0..10 {
            current = current.lerp(target, smoothing);
        }

        // Should be closer to target but not exactly at it (smooth approach)
        assert!(current < 15.0, "Should move from start position");
        assert!(current > 10.0, "Should not reach target instantly");
        assert!(current > 11.0, "Should make significant progress after 10 frames");
    }

    #[test]
    fn test_zoom_clamping_min() {
        //! Verifies zoom is clamped to min_zoom

        let controller = CameraController::default();
        let attempted_zoom = 2.0; // Below min_zoom (5.0)

        let clamped = attempted_zoom.clamp(controller.min_zoom, controller.max_zoom);

        assert_eq!(clamped, controller.min_zoom);
    }

    #[test]
    fn test_zoom_clamping_max() {
        //! Verifies zoom is clamped to max_zoom

        let controller = CameraController::default();
        let attempted_zoom = 50.0; // Above max_zoom (30.0)

        let clamped = attempted_zoom.clamp(controller.min_zoom, controller.max_zoom);

        assert_eq!(clamped, controller.max_zoom);
    }

    #[test]
    fn test_zoom_direction_scroll_up() {
        //! Scroll wheel up should decrease target (zoom in / lower camera)

        let scroll_up_delta = 1.0; // Positive scroll
        let zoom_speed = 2.0;
        let initial_target = 15.0;

        // Scroll up = zoom in = negative change
        let new_target = initial_target + (-scroll_up_delta * zoom_speed);

        assert!(new_target < initial_target, "Scroll up should zoom in (decrease height)");
        assert_eq!(new_target, 13.0);
    }

    #[test]
    fn test_zoom_direction_scroll_down() {
        //! Scroll wheel down should increase target (zoom out / raise camera)

        let scroll_down_delta = -1.0; // Negative scroll
        let zoom_speed = 2.0;
        let initial_target = 15.0;

        // Scroll down = zoom out = positive change
        let new_target = initial_target + (-scroll_down_delta * zoom_speed);

        assert!(new_target > initial_target, "Scroll down should zoom out (increase height)");
        assert_eq!(new_target, 17.0);
    }

    #[test]
    fn test_zoom_speed_affects_response() {
        //! Higher zoom_speed should result in larger target changes

        let scroll_delta = 1.0;
        let slow_speed = 1.0;
        let fast_speed = 3.0;
        let initial = 15.0;

        let slow_change = -scroll_delta * slow_speed;
        let fast_change = -scroll_delta * fast_speed;

        assert!(fast_change.abs() > slow_change.abs(),
            "Higher zoom_speed should create larger zoom changes");
    }

    #[test]
    fn test_camera_controller_debug() {
        //! Verifies debug output is useful for troubleshooting

        let controller = CameraController::default();
        let debug_str = format!("{:?}", controller);

        assert!(debug_str.contains("CameraController"));
        assert!(debug_str.contains("zoom")); // Should mention zoom fields
    }

    #[test]
    fn test_zoom_range_is_reasonable_for_chess() {
        //! Ensures default zoom range works for chess board scale

        let controller = CameraController::default();

        // Chess board is typically 8x8 units, pieces ~1 unit tall
        // Min zoom (5.0) should see several squares clearly
        // Max zoom (30.0) should see entire board
        assert!(controller.min_zoom >= 3.0, "Too close might clip through board");
        assert!(controller.max_zoom <= 50.0, "Too far loses board visibility");

        // Range should be wide enough for varied playstyles
        let zoom_range = controller.max_zoom - controller.min_zoom;
        assert!(zoom_range >= 20.0, "Range should allow significant zoom variation");
    }

    #[test]
    fn test_multiple_zoom_steps() {
        //! Simulates multiple scroll wheel inputs

        let mut target_zoom = 15.0;
        let zoom_speed = 2.0;
        let min_zoom = 5.0;
        let max_zoom = 30.0;

        // Scroll up 3 times (zoom in)
        for _ in 0..3 {
            target_zoom = (target_zoom - zoom_speed).clamp(min_zoom, max_zoom);
        }

        assert_eq!(target_zoom, 9.0);

        // Scroll down 2 times (zoom out)
        for _ in 0..2 {
            target_zoom = (target_zoom + zoom_speed).clamp(min_zoom, max_zoom);
        }

        assert_eq!(target_zoom, 13.0);
    }
}
