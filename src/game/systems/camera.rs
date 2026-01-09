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

use crate::game::resources::Selection;
use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
};
use std::f32::consts::PI;

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

    /// Camera pitch (rotation around X axis, looking up/down)
    ///
    /// Stored in radians. Clamped between -PI/2 and PI/2 to prevent gimbal lock.
    /// Negative = looking down, Positive = looking up, 0 = looking straight ahead.
    pub pitch: f32,

    /// Camera yaw (rotation around Y axis, looking left/right)
    ///
    /// Stored in radians. Wraps around at 2*PI.
    /// 0 = north, PI/2 = east, PI = south, 3*PI/2 = west.
    pub yaw: f32,

    /// Mouse rotation sensitivity multiplier
    ///
    /// Based on Bevy reference (Valorant-style): 1.0 / 180.0 radians per dot.
    /// Higher values = faster rotation response. Suggested range: 0.5-2.0.
    pub rotation_sensitivity: f32,

    /// Whether the controller has been initialized
    ///
    /// On first frame, extracts pitch/yaw from Transform rotation.
    /// Prevents sudden camera jumps on spawn.
    pub initialized: bool,
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
    /// - **pitch**: 0.0 - Extracted from Transform on first frame
    /// - **yaw**: 0.0 - Extracted from Transform on first frame
    /// - **rotation_sensitivity**: 1.0 - Valorant-style sensitivity
    /// - **initialized**: false - Will be set true after first frame
    fn default() -> Self {
        Self {
            move_speed: 12.0,
            smoothing: 0.3,
            zoom_speed: 2.0,
            zoom_smoothing: 0.15, // Slower than movement for cinematic feel
            current_zoom: 15.0,   // Typical chess board viewing height
            target_zoom: 15.0,
            min_zoom: 5.0,             // Close enough to see piece details
            max_zoom: 30.0,            // Far enough for full board overview
            pitch: 0.0,                // Will be initialized from Transform
            yaw: 0.0,                  // Will be initialized from Transform
            rotation_sensitivity: 1.0, // Bevy reference default
            initialized: false,        // Needs initialization
        }
    }
}

/// System that handles mouse wheel zoom input and updates target zoom level
///
/// Uses AccumulatedMouseScroll (Bevy 0.17+) which accumulates scroll events
/// automatically each frame. Positive delta = scroll up = zoom in (decrease height),
/// negative delta = scroll down = zoom out (increase height).
///
/// This system only updates the target; actual camera movement happens in
/// `camera_zoom_system` for smooth interpolation.
///
/// # Modern Pattern (Bevy 0.17+)
///
/// AccumulatedMouseScroll replaces the deprecated EventReader<MouseWheel> pattern.
/// It provides a single delta value per frame, already normalized across scroll types.
///
/// # Total War Feel
///
/// The zoom response is calibrated to feel like Total War games:
/// - Moderate speed (not too fast, not too slow)
/// - Each wheel tick moves target by zoom_speed units
/// - Smooth interpolation applied separately for cinematic effect
pub fn camera_zoom_input_system(
    mouse_scroll: Res<AccumulatedMouseScroll>,
    mut query: Query<&mut CameraController>,
) {
    // Only process if there was scroll input this frame
    // AccumulatedMouseScroll.delta is a Vec2 where y is vertical scroll
    if mouse_scroll.delta.y != 0.0 {
        for mut controller in query.iter_mut() {
            // Calculate zoom delta
            // AccumulatedMouseScroll.delta.y is already normalized
            // Positive y = scroll up = zoom in = decrease height
            let zoom_delta = -mouse_scroll.delta.y * controller.zoom_speed;

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
pub fn camera_zoom_system(mut query: Query<(&mut Transform, &mut CameraController)>) {
    for (mut transform, mut controller) in query.iter_mut() {
        // Smoothly interpolate current zoom toward target
        controller.current_zoom = controller
            .current_zoom
            .lerp(controller.target_zoom, controller.zoom_smoothing);

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
    selection: Res<Selection>,
    mut query: Query<(&mut Transform, &CameraController)>,
) {
    // Disable camera movement while dragging a piece
    if selection.is_dragging {
        return;
    }

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

/// Radians per mouse movement dot (based on Valorant sensitivity from Bevy reference)
pub const RADIANS_PER_DOT: f32 = 1.0 / 180.0;

/// System that handles mouse drag rotation (right-click + drag)
///
/// Implements orbit-style camera rotation following modern Bevy patterns:
/// - Uses `AccumulatedMouseMotion` (NOT multiplied by delta_time - already frame-accumulated)
/// - Pitch is clamped to prevent gimbal lock (-PI/2 to PI/2)
/// - Yaw wraps naturally at 2*PI
/// - Right mouse button must be pressed to rotate
///
/// # Modern Pattern (Bevy 0.17+)
///
/// Based on `reference/bevy/examples/helpers/camera_controller.rs`.
/// The key insight: **AccumulatedMouseMotion is already frame-accumulated**.
/// Do NOT multiply by delta_time or it will be way too slow!
///
/// # Initialization
///
/// On first frame (initialized == false), extracts current pitch/yaw from
/// the Transform's rotation to prevent sudden camera jumps.
pub fn camera_rotation_system(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    selection: Res<Selection>,
    mut query: Query<(&mut Transform, &mut CameraController)>,
) {
    // Disable camera rotation while dragging a piece
    if selection.is_dragging {
        return;
    }

    for (mut transform, mut controller) in query.iter_mut() {
        // Initialize pitch/yaw from Transform on first frame
        if !controller.initialized {
            let (yaw, pitch, _roll) = transform.rotation.to_euler(EulerRot::YXZ);
            controller.yaw = yaw;
            controller.pitch = pitch;
            controller.initialized = true;
            info!(
                "[CAMERA] Initialized - pitch: {:.2}, yaw: {:.2}",
                controller.pitch.to_degrees(),
                controller.yaw.to_degrees()
            );
        }

        let mut modified = false;

        // Keyboard Rotation (Q/E)
        // Q = Rotate Left (Increase Yaw)
        // E = Rotate Right (Decrease Yaw)
        // Speed: 2.0 radians per second (adjust as needed)
        const KEYBOARD_ROTATION_SPEED: f32 = 2.0;

        if keyboard.pressed(KeyCode::KeyQ) {
            controller.yaw += KEYBOARD_ROTATION_SPEED * time.delta_secs();
            modified = true;
        }
        if keyboard.pressed(KeyCode::KeyE) {
            controller.yaw -= KEYBOARD_ROTATION_SPEED * time.delta_secs();
            modified = true;
        }

        // Mouse Rotation (Right-click drag)
        if mouse_button.pressed(MouseButton::Right) && mouse_motion.delta != Vec2::ZERO {
            // Update pitch (up/down) with clamping to prevent gimbal lock
            // Negative delta.y = move mouse up = look up = increase pitch
            controller.pitch = (controller.pitch
                - mouse_motion.delta.y * RADIANS_PER_DOT * controller.rotation_sensitivity)
                .clamp(-PI / 2.0, PI / 2.0);

            // Update yaw (left/right) - no clamping needed, wraps naturally
            // Negative delta.x = move mouse left = look left = decrease yaw
            controller.yaw -=
                mouse_motion.delta.x * RADIANS_PER_DOT * controller.rotation_sensitivity;

            modified = true;
        }

        // Apply rotation to Transform if anything changed
        if modified {
            // Order: ZYX (roll=0, yaw, pitch) matches Bevy reference
            transform.rotation =
                Quat::from_euler(EulerRot::ZYX, 0.0, controller.yaw, controller.pitch);
        }
    }
}

/// Resource tracking camera rotation state for turn-based rotation
///
/// When a turn switches, the camera should rotate 180° around the board center
/// so each player sees the board from their perspective.
#[derive(Resource, Debug, Default)]
pub struct CameraRotationState {
    /// Target rotation angle (in radians) - 0 for White, PI for Black
    pub target_yaw: f32,

    /// Current rotation angle (in radians)
    pub current_yaw: f32,

    /// Whether rotation is in progress
    pub is_rotating: bool,

    /// Rotation speed (radians per second)
    pub rotation_speed: f32,

    /// Last turn color - used to detect turn changes
    pub last_turn_color: Option<crate::rendering::pieces::PieceColor>,
}

impl CameraRotationState {
    /// Board center position (around which we rotate)
    pub const BOARD_CENTER: Vec3 = Vec3::new(3.5, 0.0, 3.5);

    /// Rotation speed in radians per second
    pub const DEFAULT_ROTATION_SPEED: f32 = 2.0;
}

/// System that detects turn changes and initiates camera rotation
///
/// Camera stays fixed in AI mode since only one player is human.
pub fn camera_rotate_on_turn_detection_system() {
    // Camera stays fixed - always playing vs AI
}

/// System that smoothly rotates camera around board center when turn switches
///
/// Rotates the camera 180° around the Y-axis (board center) so each player
/// sees the board from their perspective. Uses smooth interpolation for
/// a cinematic rotation effect.
pub fn camera_rotate_on_turn_system(
    time: Res<Time>,
    mut rotation_state: ResMut<CameraRotationState>,
    mut camera_query: Query<&mut Transform, (With<Camera3d>, With<CameraController>)>,
) {
    if !rotation_state.is_rotating {
        return;
    }

    // Calculate rotation delta
    let delta_yaw = rotation_state.target_yaw - rotation_state.current_yaw;

    // Normalize delta to shortest path (-PI to PI range)
    let normalized_delta = {
        let mut d = delta_yaw;
        while d > PI {
            d -= 2.0 * PI;
        }
        while d < -PI {
            d += 2.0 * PI;
        }
        d
    };

    // Check if we're close enough to target (within 0.01 radians)
    if normalized_delta.abs() < 0.01 {
        rotation_state.current_yaw = rotation_state.target_yaw;
        rotation_state.is_rotating = false;
        info!(
            "[CAMERA] Rotation complete - yaw: {:.2}°",
            rotation_state.current_yaw.to_degrees()
        );
        return;
    }

    // Smoothly interpolate toward target
    let rotation_delta = normalized_delta * rotation_state.rotation_speed * time.delta_secs();
    rotation_state.current_yaw += rotation_delta;

    // Normalize current_yaw to 0-2PI range
    rotation_state.current_yaw = rotation_state.current_yaw % (2.0 * PI);
    if rotation_state.current_yaw < 0.0 {
        rotation_state.current_yaw += 2.0 * PI;
    }

    // Apply rotation to all game cameras
    for mut transform in camera_query.iter_mut() {
        // Store original position relative to board center
        let relative_pos = transform.translation - CameraRotationState::BOARD_CENTER;

        // Calculate total rotation from initial position
        // Initial yaw is 0, so we rotate by current_yaw
        let rotation_quat = Quat::from_rotation_y(rotation_state.current_yaw);

        // Rotate position around board center
        let rotated_pos = rotation_quat * relative_pos;

        // Update position
        transform.translation = CameraRotationState::BOARD_CENTER + rotated_pos;

        // Update rotation to look at board center with the new yaw
        // Extract current pitch from transform
        let (_, current_pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);

        // Create new rotation with updated yaw and same pitch
        // This maintains the viewing angle while rotating around the board
        transform.rotation = Quat::from_euler(
            EulerRot::YXZ,
            rotation_state.current_yaw,
            current_pitch,
            0.0,
        );
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

        assert!(
            controller.zoom_smoothing < controller.smoothing,
            "Zoom should be smoother (slower) than movement for cinematic effect"
        );
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
            pitch: 0.0,
            yaw: 0.0,
            rotation_sensitivity: 1.0,
            initialized: false,
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
        assert!(
            current < 12.0,
            "Should make significant progress after 10 frames"
        );
    }

    #[test]
    fn test_zoom_clamping_min() {
        //! Verifies zoom is clamped to min_zoom

        let controller = CameraController::default();
        let attempted_zoom: f32 = 2.0; // Below min_zoom (5.0)

        let clamped = attempted_zoom.clamp(controller.min_zoom, controller.max_zoom);

        assert_eq!(clamped, controller.min_zoom);
    }

    #[test]
    fn test_zoom_clamping_max() {
        //! Verifies zoom is clamped to max_zoom

        let controller = CameraController::default();
        let attempted_zoom: f32 = 50.0; // Above max_zoom (30.0)

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

        assert!(
            new_target < initial_target,
            "Scroll up should zoom in (decrease height)"
        );
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

        assert!(
            new_target > initial_target,
            "Scroll down should zoom out (increase height)"
        );
        assert_eq!(new_target, 17.0);
    }

    #[test]
    fn test_zoom_speed_affects_response() {
        //! Higher zoom_speed should result in larger target changes

        let scroll_delta: f32 = 1.0;
        let slow_speed: f32 = 1.0;
        let fast_speed: f32 = 3.0;

        let slow_change = -scroll_delta * slow_speed;
        let fast_change = -scroll_delta * fast_speed;

        assert!(
            fast_change.abs() > slow_change.abs(),
            "Higher zoom_speed should create larger zoom changes"
        );
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
        assert!(
            controller.min_zoom >= 3.0,
            "Too close might clip through board"
        );
        assert!(
            controller.max_zoom <= 50.0,
            "Too far loses board visibility"
        );

        // Range should be wide enough for varied playstyles
        let zoom_range = controller.max_zoom - controller.min_zoom;
        assert!(
            zoom_range >= 20.0,
            "Range should allow significant zoom variation"
        );
    }

    #[test]
    fn test_multiple_zoom_steps() {
        //! Simulates multiple scroll wheel inputs

        let mut target_zoom: f32 = 15.0;
        let zoom_speed: f32 = 2.0;
        let min_zoom: f32 = 5.0;
        let max_zoom: f32 = 30.0;

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

/// Configure the persistent camera for gameplay
/// Use the existing Egui camera as the main game camera to avoid conflicts
pub fn setup_game_camera(
    mut commands: Commands,
    persistent_camera: Res<crate::PersistentEguiCamera>,
    view_mode: Res<crate::game::view_mode::ViewMode>,
    mut query: Query<(&mut Transform, &mut Camera)>,
) {
    // Only configure for standard view (TempleOS handles its own camera/view)
    if *view_mode == crate::game::view_mode::ViewMode::TempleOS {
        return;
    }

    if let Some(entity) = persistent_camera.entity {
        if let Ok((mut transform, mut camera)) = query.get_mut(entity) {
            // Position for gameplay: behind White, angled down
            let initial_height = 10.0;
            let board_center = Vec3::new(3.5, 0.0, 3.5);
            let camera_pos = Vec3::new(3.5, initial_height, -8.0);

            *transform = Transform::from_translation(camera_pos).looking_at(board_center, Vec3::Y);

            // Ensure order is correct (0 is standard for 3D)
            camera.order = 0;

            // Add RTS camera controls
            commands.entity(entity).insert(CameraController {
                current_zoom: initial_height,
                target_zoom: initial_height,
                min_zoom: 3.0,
                max_zoom: 30.0,
                ..Default::default()
            });

            info!("[CAMERA] Configured Persistent Camera for Gameplay");
        }
    }
}

/// Reset the persistent camera when exiting gameplay
pub fn reset_game_camera(
    mut commands: Commands,
    persistent_camera: Res<crate::PersistentEguiCamera>,
    mut query: Query<&mut Camera>,
) {
    if let Some(entity) = persistent_camera.entity {
        // Remove RTS controls
        commands.entity(entity).remove::<CameraController>();

        // Reset order if needed (though 0 is usually fine for menus too)
        if let Ok(mut camera) = query.get_mut(entity) {
            camera.order = 0;
        }

        info!("[CAMERA] Reset Persistent Camera (Removed Controls)");
    }
}
