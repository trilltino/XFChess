//! Camera view modes for cycling through different perspectives
//!
//! Provides 5 camera modes that cycle with the 'R' key:
//! - TopDownWhite: 90° overhead, White pieces at bottom
//! - TopDownBlack: 90° overhead, Black pieces at bottom
//! - Fixed: Static angled view, all controls disabled
//! - Default: Standard 3D perspective view
//! - Cinematic: Elaborate sequence with elliptical orbits and varying zoom

use bevy::prelude::*;

/// Camera view modes that cycle with 'R' key
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Resource, Default)]
#[reflect(Resource)]
pub enum CameraViewMode {
    /// 90° overhead view with White pieces facing bottom
    #[default]
    TopDownWhite,
    /// 90° overhead view with Black pieces facing bottom
    TopDownBlack,
    /// Static angled view at board center, all controls disabled
    Fixed,
    /// Standard 3D perspective view (existing behavior)
    Default,
    /// Elaborate cinematic sequence with elliptical paths
    Cinematic,
}

impl CameraViewMode {
    /// Get the next mode in the cycle
    pub fn next(self) -> Self {
        match self {
            CameraViewMode::TopDownWhite => CameraViewMode::TopDownBlack,
            CameraViewMode::TopDownBlack => CameraViewMode::Fixed,
            CameraViewMode::Fixed => CameraViewMode::Default,
            CameraViewMode::Default => CameraViewMode::Cinematic,
            CameraViewMode::Cinematic => CameraViewMode::TopDownWhite,
        }
    }

    /// Check if this mode disables camera controls
    pub fn is_fixed(self) -> bool {
        matches!(self, CameraViewMode::Fixed | CameraViewMode::Cinematic)
    }
}

/// Transition type for cinematic camera movement
#[derive(Debug, Clone, Copy)]
pub enum TransitionType {
    /// Linear interpolation between positions
    Linear,
    /// Ease in-out smoothing
    EaseInOut,
    /// Elliptical orbital path
    Elliptical {
        /// Center point of the ellipse
        center: Vec3,
        /// X axis of the ellipse (horizontal)
        axis_x: f32,
        /// Z axis of the ellipse (depth)
        axis_z: f32,
        /// Starting angle in radians
        start_angle: f32,
        /// Ending angle in radians
        end_angle: f32,
    },
}

/// A single keyframe in the cinematic sequence
#[derive(Debug, Clone, Copy)]
pub struct CinematicKeyFrame {
    /// Target position for this keyframe
    pub position: Vec3,
    /// Point to look at
    pub look_at: Vec3,
    /// Duration to reach this keyframe from previous
    pub duration_secs: f32,
    /// Type of transition to use
    pub transition_type: TransitionType,
    /// Target zoom level (camera height)
    pub target_zoom: f32,
}

/// Resource tracking cinematic sequence state
#[derive(Resource, Debug)]
pub struct CinematicSequence {
    /// All keyframes in the sequence
    pub keyframes: Vec<CinematicKeyFrame>,
    /// Current keyframe index
    pub current_frame: usize,
    /// Time elapsed in current frame
    pub elapsed_in_frame: f32,
    /// Whether currently fading to black
    pub is_fading: bool,
    /// Fade progress (0.0 = visible, 1.0 = fully black)
    pub fade_progress: f32,
    /// Fade direction: true = fading out, false = fading in
    pub fade_out: bool,
    /// Time spent fading
    pub fade_time: f32,
    /// Total fade duration
    pub fade_duration: f32,
}

impl Default for CinematicSequence {
    fn default() -> Self {
        Self {
            keyframes: Self::create_default_sequence(),
            current_frame: 0,
            elapsed_in_frame: 0.0,
            is_fading: false,
            fade_progress: 0.0,
            fade_out: false,
            fade_time: 0.0,
            fade_duration: 1.5,
        }
    }
}

impl CinematicSequence {
    /// Create the default elaborate cinematic sequence
    fn create_default_sequence() -> Vec<CinematicKeyFrame> {
        let board_center = Vec3::new(3.5, 0.0, 3.5);
        
        vec![
            // Phase 1: Low sweep from white side (kingside to queenside)
            CinematicKeyFrame {
                position: Vec3::new(6.0, 6.0, -4.0),
                look_at: board_center,
                duration_secs: 4.0,
                transition_type: TransitionType::Elliptical {
                    center: Vec3::new(3.5, 5.0, 0.0),
                    axis_x: 5.0,
                    axis_z: 4.0,
                    start_angle: -0.5,
                    end_angle: 0.5,
                },
                target_zoom: 6.0,
            },
            // Continue sweep to queenside corner
            CinematicKeyFrame {
                position: Vec3::new(1.0, 7.0, -2.0),
                look_at: board_center,
                duration_secs: 3.0,
                transition_type: TransitionType::EaseInOut,
                target_zoom: 7.0,
            },
            // Phase 2: Rise and rotate to corner view
            CinematicKeyFrame {
                position: Vec3::new(-3.0, 12.0, -3.0),
                look_at: board_center,
                duration_secs: 4.0,
                transition_type: TransitionType::Elliptical {
                    center: Vec3::new(0.0, 8.0, 0.0),
                    axis_x: 4.0,
                    axis_z: 4.0,
                    start_angle: 0.0,
                    end_angle: 1.57,
                },
                target_zoom: 12.0,
            },
            // Phase 3: Cross-board diagonal (with fade)
            CinematicKeyFrame {
                position: Vec3::new(10.0, 15.0, 10.0),
                look_at: board_center,
                duration_secs: 5.0,
                transition_type: TransitionType::Elliptical {
                    center: board_center,
                    axis_x: 8.0,
                    axis_z: 8.0,
                    start_angle: 2.36,
                    end_angle: 3.93,
                },
                target_zoom: 15.0,
            },
            // Phase 4: Orbital rotation at medium height
            CinematicKeyFrame {
                position: Vec3::new(3.5, 10.0, -6.0),
                look_at: board_center,
                duration_secs: 6.0,
                transition_type: TransitionType::Elliptical {
                    center: board_center,
                    axis_x: 6.0,
                    axis_z: 6.0,
                    start_angle: -1.57,
                    end_angle: 1.57,
                },
                target_zoom: 10.0,
            },
            // Phase 5: Rise to high view while continuing orbit
            CinematicKeyFrame {
                position: Vec3::new(3.5, 18.0, 9.0),
                look_at: board_center,
                duration_secs: 5.0,
                transition_type: TransitionType::Elliptical {
                    center: board_center,
                    axis_x: 6.0,
                    axis_z: 6.0,
                    start_angle: 1.57,
                    end_angle: 4.71,
                },
                target_zoom: 18.0,
            },
            // Phase 6: Dramatic dive to center
            CinematicKeyFrame {
                position: Vec3::new(3.5, 4.0, 3.5),
                look_at: Vec3::new(3.5, 0.0, 5.0),
                duration_secs: 4.0,
                transition_type: TransitionType::EaseInOut,
                target_zoom: 4.0,
            },
            // Phase 7: Low sweep across board surface
            CinematicKeyFrame {
                position: Vec3::new(0.0, 3.0, 3.5),
                look_at: Vec3::new(7.0, 2.0, 3.5),
                duration_secs: 4.0,
                transition_type: TransitionType::Linear,
                target_zoom: 3.0,
            },
            // Phase 8: Pull back to starting position
            CinematicKeyFrame {
                position: Vec3::new(6.0, 6.0, -4.0),
                look_at: board_center,
                duration_secs: 5.0,
                transition_type: TransitionType::Elliptical {
                    center: Vec3::new(3.5, 5.0, 3.5),
                    axis_x: 5.0,
                    axis_z: 5.0,
                    start_angle: 3.14,
                    end_angle: -0.5,
                },
                target_zoom: 6.0,
            },
        ]
    }

    /// Reset sequence to beginning
    pub fn reset(&mut self) {
        self.current_frame = 0;
        self.elapsed_in_frame = 0.0;
        self.is_fading = false;
        self.fade_progress = 0.0;
        self.fade_out = false;
        self.fade_time = 0.0;
    }

    /// Check if we should trigger a fade at current frame transition
    pub fn should_fade_at_transition(&self) -> bool {
        // Fade after phase 3 and phase 6
        self.current_frame == 3 || self.current_frame == 6
    }
}

/// Resource for fade overlay during cinematic transitions
#[derive(Resource, Debug, Default)]
pub struct CinematicFadeOverlay {
    /// Entity ID of the fade UI element
    #[allow(dead_code)]
    pub entity: Option<Entity>,
}

/// Component for tracking if camera controls should be disabled
#[derive(Component, Debug, Default)]
pub struct CameraControlsDisabled;
