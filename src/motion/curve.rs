//! The one movement evaluator every piece animation samples: eased progress,
//! a lift arc that is zero at both ends, and the midpoint/completion phases.
//! It is pure, so gameplay, replay, the menu showcase and the LEARN board all
//! share it, and it is testable without a window.

use bevy::curve::{easing::EaseFunction, Curve};
use bevy::math::{Vec2, Vec3};

/// Easing applied to horizontal travel. Cubic smoothstep never overshoots.
pub const DEFAULT_EASING: EaseFunction = EaseFunction::SmoothStep;

/// Normalised progress at which a visual transaction becomes `Committed`.
pub const COMMIT_PROGRESS: f32 = 0.5;

/// Seconds a gameplay move takes, in both the 2D and 3D views.
pub const GAMEPLAY_MOVE_SECONDS: f32 = 0.4;

/// How high a piece lifts at the midpoint of a move.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArcHeight {
    Fixed(f32),
    /// `distance * factor`, clamped, so short hops stay low.
    PerDistance {
        factor: f32,
        min: f32,
        max: f32,
    },
}

impl ArcHeight {
    pub fn for_distance(self, distance: f32) -> f32 {
        let finite_height = |height: f32| {
            if height.is_finite() {
                height.max(0.0)
            } else {
                0.0
            }
        };
        match self {
            ArcHeight::Fixed(height) => finite_height(height),
            ArcHeight::PerDistance { factor, min, max } => {
                let distance = finite_height(distance);
                let (min, max) = (finite_height(min), finite_height(max));
                (distance * finite_height(factor)).clamp(min.min(max), min.max(max))
            }
        }
    }
}

/// Easing and lift for one kind of board; durations are chosen per move.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionStyle {
    pub easing: EaseFunction,
    pub arc: ArcHeight,
}

impl MotionStyle {
    /// Live games, in both views and in replay.
    pub const GAMEPLAY: MotionStyle = MotionStyle {
        easing: DEFAULT_EASING,
        arc: ArcHeight::PerDistance {
            factor: 0.18,
            min: 0.15,
            max: 0.55,
        },
    };
    /// The main-menu showcase board.
    pub const MENU: MotionStyle = MotionStyle {
        easing: DEFAULT_EASING,
        arc: ArcHeight::Fixed(0.28),
    };
    /// The miniature LEARN board.
    pub const MINI: MotionStyle = MotionStyle {
        easing: DEFAULT_EASING,
        arc: ArcHeight::Fixed(0.22),
    };

    /// Samples the motion `elapsed` seconds into a move of `duration` seconds
    /// covering `distance` board units.
    pub fn sample(&self, elapsed: f32, duration: f32, distance: f32) -> MotionSample {
        let progress = normalized_progress(elapsed, duration);
        let eased_progress = self.easing.sample_clamped(progress).clamp(0.0, 1.0);
        // The arc follows linear progress so its peak is always the midpoint.
        let lift = self.arc.for_distance(distance) * 4.0 * progress * (1.0 - progress);
        MotionSample {
            progress,
            eased_progress,
            lift,
            committed: progress >= COMMIT_PROGRESS,
            complete: progress >= 1.0,
        }
    }
}

/// Where a move is at one instant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionSample {
    /// Linear progress in `0..=1`.
    pub progress: f32,
    /// Eased progress in `0..=1`; this drives horizontal travel.
    pub eased_progress: f32,
    /// Height above the board, zero at both endpoints.
    pub lift: f32,
    /// Past the midpoint.
    pub committed: bool,
    pub complete: bool,
}

impl MotionSample {
    /// Board-space position; exactly `to` once complete.
    pub fn position2(&self, from: Vec2, to: Vec2) -> Vec2 {
        if self.complete {
            to
        } else {
            from.lerp(to, self.eased_progress)
        }
    }

    /// World-space position with the lift added on Y; exactly `to` once complete.
    pub fn position3(&self, from: Vec3, to: Vec3) -> Vec3 {
        if self.complete {
            to
        } else {
            from.lerp(to, self.eased_progress) + Vec3::Y * self.lift
        }
    }
}

/// `elapsed / duration` in `0..=1`. A non-finite elapsed time counts as not
/// started; a non-positive or non-finite duration counts as already finished.
pub fn normalized_progress(elapsed: f32, duration: f32) -> f32 {
    if !duration.is_finite() || duration <= 0.0 {
        return 1.0;
    }
    if !elapsed.is_finite() {
        return 0.0;
    }
    (elapsed / duration).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const STYLE: MotionStyle = MotionStyle::GAMEPLAY;

    #[test]
    fn endpoints_are_exact() {
        let from = Vec2::new(6.0, 0.0);
        let to = Vec2::new(5.0, 2.0);
        let start = STYLE.sample(0.0, 0.4, 2.2);
        let end = STYLE.sample(0.4, 0.4, 2.2);
        assert_eq!(start.position2(from, to), from);
        assert_eq!(end.position2(from, to), to);
        assert_eq!(start.lift, 0.0);
        assert_eq!(end.lift, 0.0);
        assert!(!start.committed && !start.complete);
        assert!(end.committed && end.complete);
    }

    #[test]
    fn midpoint_is_committed_and_peaks_the_lift() {
        let mid = STYLE.sample(0.2, 0.4, 3.0);
        assert!(mid.committed);
        assert!(!mid.complete);
        assert!((mid.progress - 0.5).abs() < 1e-6);
        assert!((mid.lift - 0.54).abs() < 1e-6);
        assert!((mid.eased_progress - 0.5).abs() < 1e-6);
    }

    #[test]
    fn eased_progress_is_monotonic_and_never_overshoots() {
        let mut previous = 0.0;
        for step in 0..=1000 {
            let sample = STYLE.sample(step as f32 / 1000.0, 1.0, 2.0);
            assert!(sample.eased_progress >= previous);
            assert!((0.0..=1.0).contains(&sample.eased_progress));
            assert!(sample.lift.is_finite() && sample.lift >= 0.0);
            previous = sample.eased_progress;
        }
        assert_eq!(previous, 1.0);
    }

    #[test]
    fn invalid_inputs_do_not_panic_or_overshoot() {
        for duration in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(STYLE.sample(0.1, duration, 1.0).complete);
        }
        for elapsed in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -5.0] {
            let sample = STYLE.sample(elapsed, 0.4, 1.0);
            assert!(sample.progress == 0.0 || sample.progress == 1.0);
            assert!(sample.lift.is_finite());
        }
        let huge = STYLE.sample(1.0e9, 0.4, 1.0);
        assert!(huge.complete);
        assert_eq!(huge.progress, 1.0);
        assert_eq!(huge.lift, 0.0);
    }

    #[test]
    fn arc_height_scales_with_distance_within_bounds() {
        let arc = MotionStyle::GAMEPLAY.arc;
        assert_eq!(arc.for_distance(0.0), 0.15);
        assert_eq!(arc.for_distance(100.0), 0.55);
        assert_eq!(arc.for_distance(f32::NAN), 0.15);
        assert_eq!(MotionStyle::MENU.arc.for_distance(5.0), 0.28);
    }

    #[test]
    fn world_position_adds_lift_on_y_only() {
        let from = Vec3::new(0.0, 0.1, 0.0);
        let to = Vec3::new(2.0, 0.1, 0.0);
        let mid = MotionStyle::MENU.sample(0.5, 1.0, 2.0);
        let position = mid.position3(from, to);
        assert!((position.x - 1.0).abs() < 1e-6);
        assert!((position.y - (0.1 + 0.28)).abs() < 1e-6);
        assert_eq!(position.z, 0.0);
    }
}
