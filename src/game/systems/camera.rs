#![allow(dead_code)]
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
//! - **Q**: Rotate camera left
//! - **E**: Rotate camera right
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

use crate::core::states::GameMode;
use crate::game::camera_modes::{CameraViewMode, CinematicSequence, TransitionType, CameraControlsDisabled};
use crate::game::resources::{CurrentTurn, Selection, Players};
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
        let up = transform.up();

        // Project onto XZ plane (maintain Y height for RTS-style movement)
        // CRITICAL FIX: when looking straight down (Forward = -Y), Forward.xz is zero!
        // In that case, we must use the UP vector (which points to Board "North") for forward movement.
        let is_vertical = forward.y.abs() > 0.9;

        // When vertical (top-down), use UP vector for forward/backward
        // When angled (perspective), use projected FORWARD vector
        let forward_xz = if is_vertical {
            Vec3::new(up.x, 0.0, up.z).normalize_or_zero()
        } else {
            Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero()
        };

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

/// Helper to determine if the camera should show the Black player's perspective
pub fn get_is_black_view(
    players: &Players,
    current_turn: &CurrentTurn,
    game_mode: GameMode,
) -> bool {
    // In local multiplayer, flip based on whose turn it is
    if game_mode == GameMode::MultiplayerLocal {
        return current_turn.color == crate::rendering::pieces::PieceColor::Black;
    }

    // In other modes (SinglePlayer vs AI, Online), fix to the human player's perspective
    if players.player_1.is_human && players.player_1.color == crate::rendering::pieces::PieceColor::Black {
        return true;
    }
    if players.player_2.is_human && players.player_2.color == crate::rendering::pieces::PieceColor::Black {
        return true;
    }

    false
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
/// In local PvP mode, rotates camera 180° so each player sees the board from their side.
/// In AI or online multiplayer mode, camera stays fixed on the human player's perspective.
pub fn camera_rotate_on_turn_detection_system(
    current_turn: Res<CurrentTurn>,
    players: Res<Players>,
    game_mode: Res<GameMode>,
    mut rotation_state: ResMut<CameraRotationState>,
) {
    use crate::rendering::pieces::PieceColor;

    // Detect turn change or initial setup
    let turn_color = current_turn.color;

    if rotation_state.last_turn_color == Some(turn_color) {
        return; // No change in turn
    }

    let is_black_view = get_is_black_view(&players, &current_turn, *game_mode);
    let is_local_pvp = *game_mode == GameMode::MultiplayerLocal;

    if is_local_pvp {
        // In local PvP, rotate camera every turn
        rotation_state.target_yaw = if is_black_view { PI } else { 0.0 };
        rotation_state.last_turn_color = Some(turn_color);
        rotation_state.rotation_speed = CameraRotationState::DEFAULT_ROTATION_SPEED;
        rotation_state.is_rotating = true;
    } else if rotation_state.last_turn_color.is_none() {
        // At start of game in other modes, rotate to human player's side
        let target_yaw = if is_black_view { PI } else { 0.0 };
        
        rotation_state.target_yaw = target_yaw;
        rotation_state.last_turn_color = Some(turn_color);
        
        // If we need to rotate initially, do it instantly or smoothly
        if (target_yaw - rotation_state.current_yaw).abs() > 0.01 {
            rotation_state.rotation_speed = CameraRotationState::DEFAULT_ROTATION_SPEED * 2.0; // Faster initial rotation
            rotation_state.is_rotating = true;
        }
    } else {
        // In single player/network, don't rotate on turn changes after initial setup
        rotation_state.last_turn_color = Some(turn_color);
    }
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
    players: Res<Players>,
    current_turn: Res<CurrentTurn>,
    game_mode: Res<GameMode>,
    ai_config: Res<crate::game::ai::ChessAIResource>,
) {
    // Only configure for standard views (TempleOS handles its own camera/view)
    if *view_mode == crate::game::view_mode::ViewMode::TempleOS {
        return;
    }

    let is_2d = *view_mode == crate::game::view_mode::ViewMode::Standard2D;

    if let Some(entity) = persistent_camera.entity {
        if let Ok((mut transform, mut camera)) = query.get_mut(entity) {
            // Position for gameplay: Standard Chess Perspective
            // Raised camera angle (55-65° elevation) for better board readability
            // Higher elevation reduces back-rank compression while keeping silhouettes visible
            let initial_height = 16.0;
            let distance_behind = 8.0; // Distance behind the board edge

            // Determine if the human player is Black:
            let is_black_view = get_is_black_view(&players, &current_turn, *game_mode);

            let board_center = Vec3::new(3.5, 0.0, 3.5);

            // Always start at the white-side reference position (yaw = 0).
            // CameraRotationState handles rotating to the black side when needed,
            // so the two systems stay in sync and don't fight each other.
            let camera_pos = Vec3::new(3.5, initial_height, -distance_behind);

            if is_2d {
                // Top-down 2D View
                // Position directly above center, looking straight down
                let height = 12.0;
                let translation = Vec3::new(3.5, height, 3.5);
                
                let up_vec = if is_black_view {
                    Vec3::new(-1.0, 0.0, 0.0)
                } else {
                    Vec3::new(1.0, 0.0, 0.0)
                };
                
                *transform = Transform::from_translation(translation).looking_at(board_center, up_vec);
            } else {
                // Standard 3D Perspective
                // Camera looks toward board center with correct orientation
                *transform = Transform::from_translation(camera_pos).looking_at(board_center, Vec3::Y);
            }

            // Ensure order is correct (0 is standard for 3D)
            camera.order = 0;

            // Add RTS camera controls
            // Check if controller already exists to preserve zoom/state if re-running?
            // Usually setup runs once on Enter.
            commands.entity(entity).insert(CameraController {
                current_zoom: transform.translation.y,
                target_zoom: transform.translation.y,
                min_zoom: if is_2d { 5.0 } else { 3.0 },
                max_zoom: if is_2d { 20.0 } else { 30.0 },
                // Let the system calculate pitch/yaw from the Transform we just set
                initialized: false,
                ..Default::default()
            });

            info!(
                "[CAMERA] Configured Persistent Camera. is_black_view: {} (rotation system will handle flip). Pos: {:?}",
                is_black_view, camera_pos
            );
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

/// System to reset camera to default "Standard Perspective" when 'N' is pressed
pub fn camera_reset_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    players: Res<Players>,
    current_turn: Res<CurrentTurn>,
    game_mode: Res<GameMode>,
    mut query: Query<(&mut Transform, &mut CameraController)>,
) {
    if keyboard.just_pressed(KeyCode::KeyN) {
        // Player color detection enabled
        let is_black_view = get_is_black_view(&players, &current_turn, *game_mode);

        for (mut transform, mut controller) in query.iter_mut() {
            // Standard Perspective defaults
            // Position camera along X-axis for proper chess board orientation
            let initial_height = 16.0;
            let distance_behind = 8.0;
            let board_center = Vec3::new(3.5, 0.0, 3.5);
            let default_zoom = 16.0;
            
            let default_pos = if is_black_view {
                // Black view: camera on +Z side looking toward -Z
                Vec3::new(3.5, initial_height, 7.0 + distance_behind)
            } else {
                // White view: camera on -Z side looking toward +Z
                Vec3::new(3.5, initial_height, -distance_behind)
            };

            *transform = Transform::from_translation(default_pos).looking_at(board_center, Vec3::Y);

            controller.current_zoom = default_zoom;
            controller.target_zoom = default_zoom;
            // Yaw is calculated from transform automatically when initialized=false
            controller.initialized = false;

            info!("[CAMERA] Reset to {} Perspective with correct board orientation", if is_black_view { "Black" } else { "White" });
        }
    }
}

/// System to handle 'V' key for toggling view mode during gameplay
pub fn view_mode_toggle_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut view_preferences: ResMut<crate::game::view_mode::PlayerViewPreferences>,
    mut view_mode: ResMut<crate::game::view_mode::ViewMode>,
    // We need to trigger camera re-setup
    commands: Commands,
    persistent_camera: Res<crate::PersistentEguiCamera>,
    query: Query<(&mut Transform, &mut Camera)>,
    players: Res<Players>,
    current_turn: Res<CurrentTurn>,
    game_mode: Res<GameMode>,
    ai_config: Res<crate::game::ai::ChessAIResource>,
) {
    if keyboard.just_pressed(KeyCode::KeyV) {
        view_preferences.toggle_view();
        *view_mode = view_preferences.local_view;
        info!("[VIEW] Toggled view mode to {:?}", *view_mode);
        
        // Re-run camera setup logic for the new mode
        setup_game_camera(
            commands,
            persistent_camera,
            view_mode.into(), // Convert ResMut to Res
            query,
            players,
            current_turn,
            game_mode,
            ai_config,
        );
    }
}

/// System to cycle through camera view modes with 'R' key
pub fn camera_mode_cycle_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut camera_view_mode: ResMut<CameraViewMode>,
    mut cinematic_sequence: ResMut<CinematicSequence>,
    mut commands: Commands,
    persistent_camera: Res<crate::PersistentEguiCamera>,
    mut query: Query<(&mut Transform, &mut CameraController), With<Camera3d>>,
    players: Res<Players>,
    current_turn: Res<CurrentTurn>,
    game_mode: Res<GameMode>,
    ai_config: Res<crate::game::ai::ChessAIResource>,
) {
    if keyboard.just_pressed(KeyCode::KeyR) {
        let next_mode = camera_view_mode.next();
        *camera_view_mode = next_mode;
        info!("[CAMERA_MODE] Switched to {:?}", next_mode);

        // Reset cinematic sequence when entering or leaving cinematic mode
        if next_mode == CameraViewMode::Cinematic {
            cinematic_sequence.reset();
        }

        // Apply camera position for the new mode
        if let Some(camera_entity) = persistent_camera.entity {
            if let Ok((mut transform, mut controller)) = query.get_mut(camera_entity) {
                let board_center = Vec3::new(3.5, 0.0, 3.5);

                match next_mode {
                    CameraViewMode::TopDownWhite => {
                        // 90° overhead, white at bottom
                        let height = 12.0;
                        let translation = Vec3::new(3.5, height, 3.5);
                        let up_vec = Vec3::new(1.0, 0.0, 0.0);
                        *transform = Transform::from_translation(translation).looking_at(board_center, up_vec);
                        controller.target_zoom = height;
                        controller.current_zoom = height;
                        commands.entity(camera_entity).remove::<CameraControlsDisabled>();
                    }
                    CameraViewMode::TopDownBlack => {
                        // 90° overhead, black at bottom
                        let height = 12.0;
                        let translation = Vec3::new(3.5, height, 3.5);
                        let up_vec = Vec3::new(-1.0, 0.0, 0.0);
                        *transform = Transform::from_translation(translation).looking_at(board_center, up_vec);
                        controller.target_zoom = height;
                        controller.current_zoom = height;
                        commands.entity(camera_entity).remove::<CameraControlsDisabled>();
                    }
                    CameraViewMode::Fixed => {
                        // Static angled view at center
                        let height = 14.0;
                        let distance = 6.0;
                        
                        // Determine player color for orientation
                        let is_black_view = get_is_black_view(&players, &current_turn, *game_mode);

                        let camera_pos = if is_black_view {
                            Vec3::new(3.5, height, 7.0 + distance)
                        } else {
                            Vec3::new(3.5, height, -distance)
                        };

                        *transform = Transform::from_translation(camera_pos).looking_at(board_center, Vec3::Y);
                        controller.target_zoom = height;
                        controller.current_zoom = height;
                        commands.entity(camera_entity).insert(CameraControlsDisabled);
                    }
                    CameraViewMode::Default => {
                        // Standard 3D perspective - same as game setup
                        let initial_height = 16.0;
                        let distance_behind = 8.0;
                        
                        let is_black_view = get_is_black_view(&players, &current_turn, *game_mode);

                        let camera_pos = if is_black_view {
                            Vec3::new(3.5, initial_height, 7.0 + distance_behind)
                        } else {
                            Vec3::new(3.5, initial_height, -distance_behind)
                        };

                        *transform = Transform::from_translation(camera_pos).looking_at(board_center, Vec3::Y);
                        controller.target_zoom = initial_height;
                        controller.current_zoom = initial_height;
                        controller.initialized = false;
                        commands.entity(camera_entity).remove::<CameraControlsDisabled>();
                    }
                    CameraViewMode::Cinematic => {
                        // Cinematic mode - controls disabled, sequence takes over
                        commands.entity(camera_entity).insert(CameraControlsDisabled);
                    }
                }
            }
        }
    }
}

/// Component marker for cinematic fade overlay
#[derive(Component)]
pub struct CinematicFadeOverlayComponent;

/// System to update cinematic camera with elaborate movements
pub fn cinematic_camera_system(
    time: Res<Time>,
    mut sequence: ResMut<CinematicSequence>,
    camera_view_mode: Res<CameraViewMode>,
    mut camera_query: Query<(&mut Transform, &mut CameraController), With<Camera3d>>,
    mut commands: Commands,
    fade_query: Query<Entity, With<CinematicFadeOverlayComponent>>,
    mut color_query: Query<&mut BackgroundColor>,
) {
    if *camera_view_mode != CameraViewMode::Cinematic {
        // Remove any existing fade overlay when not in cinematic mode
        for entity in fade_query.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }

    let dt = time.delta_secs();
    let num_frames = sequence.keyframes.len();
    let current_frame_idx = sequence.current_frame % num_frames;
    let next_frame_idx = (sequence.current_frame + 1) % num_frames;

    // Copy frame data to avoid borrow issues
    let current_frame = sequence.keyframes[current_frame_idx].clone();
    let next_frame = sequence.keyframes[next_frame_idx].clone();
    let should_fade = sequence.should_fade_at_transition();

    // Handle fading - spawn fade overlay if needed
    if sequence.is_fading {
        sequence.fade_time += dt;
        let fade_t = (sequence.fade_time / sequence.fade_duration).clamp(0.0, 1.0);

        // Ensure fade overlay exists
        if fade_query.is_empty() {
            commands.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
                ZIndex(1000), // High z-index to cover everything
                CinematicFadeOverlayComponent,
                Name::new("Cinematic Fade Overlay"),
            ));
        }

        if sequence.fade_out {
            // Fading out (to black)
            sequence.fade_progress = fade_t;
            if fade_t >= 1.0 {
                // Finished fade out, start fade in
                sequence.fade_out = false;
                sequence.fade_time = 0.0;
                sequence.current_frame = next_frame_idx;
            }
        } else {
            // Fading in (from black)
            sequence.fade_progress = 1.0 - fade_t;
            if fade_t >= 1.0 {
                // Finished fade in
                sequence.is_fading = false;
                sequence.fade_progress = 0.0;
                sequence.fade_time = 0.0;
            }
        }

        // Update fade overlay color
        for entity in fade_query.iter() {
            if let Ok(mut bg) = color_query.get_mut(entity) {
                *bg = BackgroundColor(Color::srgba(0.0, 0.0, 0.0, sequence.fade_progress));
            }
        }
        return;
    } else {
        // Remove fade overlay when not fading
        for entity in fade_query.iter() {
            commands.entity(entity).despawn();
        }
    }

    // Check if we need to start a fade at this transition
    sequence.elapsed_in_frame += dt;
    if sequence.elapsed_in_frame >= current_frame.duration_secs && should_fade {
        sequence.is_fading = true;
        sequence.fade_out = true;
        sequence.fade_time = 0.0;
        sequence.fade_progress = 0.0;
        return;
    }

    // Normal interpolation
    let t = (sequence.elapsed_in_frame / current_frame.duration_secs).min(1.0);
    let smooth_t = t * t * (3.0 - 2.0 * t); // Smooth step interpolation

    for (mut transform, mut controller) in camera_query.iter_mut() {
        // Calculate position based on transition type
        let new_position = match current_frame.transition_type {
            TransitionType::Linear => {
                current_frame.position.lerp(next_frame.position, smooth_t)
            }
            TransitionType::EaseInOut => {
                // Cubic ease in-out
                let ease_t = if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - ((-2.0 * t + 2.0).powi(3) / 2.0)
                };
                current_frame.position.lerp(next_frame.position, ease_t)
            }
            TransitionType::Elliptical { center, axis_x, axis_z, start_angle, end_angle } => {
                // Interpolate angle
                let angle = start_angle + (end_angle - start_angle) * smooth_t;
                
                // Calculate position on ellipse
                let x = center.x + axis_x * angle.cos();
                let z = center.z + axis_z * angle.sin();
                
                // Interpolate height separately
                let y = current_frame.position.lerp(next_frame.position, smooth_t).y;
                
                Vec3::new(x, y, z)
            }
        };

        // Calculate look_at (can also be interpolated for smooth transitions)
        let new_look_at = current_frame.look_at.lerp(next_frame.look_at, smooth_t);

        // Interpolate zoom/height
        let target_zoom = current_frame.target_zoom.lerp(next_frame.target_zoom, smooth_t);

        // Apply transform
        *transform = Transform::from_translation(new_position).looking_at(new_look_at, Vec3::Y);
        controller.target_zoom = target_zoom;
        controller.current_zoom = target_zoom;

        // Advance to next frame when complete
        if sequence.elapsed_in_frame >= current_frame.duration_secs && !sequence.is_fading {
            sequence.elapsed_in_frame = 0.0;
            sequence.current_frame = next_frame_idx;
            info!("[CINEMATIC] Advanced to keyframe {}", sequence.current_frame);
        }
    }
}

/// Run condition to check if camera controls are enabled
pub fn camera_controls_enabled(
    camera_view_mode: Res<CameraViewMode>,
    query: Query<(), (With<Camera3d>, With<CameraControlsDisabled>)>,
) -> bool {
    !camera_view_mode.is_fixed() && query.is_empty()
}
