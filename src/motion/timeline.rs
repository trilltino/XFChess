//! A visual transaction: one piece travelling between two board positions.
//!
//! "Committed" and "Completed" describe only the animation. They say nothing
//! about whether the chess move is legal, accepted, broadcast or recorded; the
//! authoritative state is updated before any transaction starts.

use super::curve::{MotionSample, MotionStyle};
use bevy::math::Vec2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisualTransactionStatus {
    /// Before the midpoint.
    Uncommitted,
    /// Past the midpoint, still travelling.
    Committed,
    Completed,
}

/// What a [`MoveTransaction::advance`] call crossed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Crossed {
    pub committed: bool,
    pub completed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoveTransaction {
    /// Board-space (file, rank) start; may be fractional after a retarget.
    pub from: Vec2,
    pub to: Vec2,
    pub elapsed: f32,
    pub duration: f32,
    pub status: VisualTransactionStatus,
    pub style: MotionStyle,
    /// Preserves height when a new accepted move interrupts this one.
    pub start_lift: f32,
}

impl MoveTransaction {
    pub fn new(from: Vec2, to: Vec2, duration: f32, style: MotionStyle) -> Self {
        Self {
            from,
            to,
            elapsed: 0.0,
            duration,
            status: VisualTransactionStatus::Uncommitted,
            style,
            start_lift: 0.0,
        }
    }

    pub fn sample(&self) -> MotionSample {
        let mut sample =
            self.style
                .sample(self.elapsed, self.duration, self.from.distance(self.to));
        // Retargets preserve height without accumulating lift under a burst of
        // accepted moves. Both terms reach zero exactly at the destination.
        sample.lift = sample
            .lift
            .max(self.start_lift * (1.0 - sample.eased_progress));
        sample
    }

    /// Board-space position right now.
    pub fn position(&self) -> Vec2 {
        self.sample().position2(self.from, self.to)
    }

    /// Height above the board right now.
    pub fn lift(&self) -> f32 {
        self.sample().lift
    }

    pub fn is_complete(&self) -> bool {
        self.status == VisualTransactionStatus::Completed
    }

    /// Moves time forward and reports the phases passed. Each phase is
    /// reported once, even when a single frame skips past the midpoint.
    pub fn advance(&mut self, delta_secs: f32) -> Crossed {
        let mut crossed = Crossed::default();
        if self.is_complete() {
            return crossed;
        }
        if delta_secs.is_finite() && delta_secs > 0.0 {
            if !self.elapsed.is_finite() {
                self.elapsed = 0.0;
            }
            self.elapsed = (self.elapsed.max(0.0) + delta_secs).min(f32::MAX);
        }
        let sample = self.sample();
        if self.status == VisualTransactionStatus::Uncommitted && sample.committed {
            self.status = VisualTransactionStatus::Committed;
            crossed.committed = true;
        }
        if sample.complete {
            self.status = VisualTransactionStatus::Completed;
            crossed.completed = true;
        }
        crossed
    }

    /// Redirects the move to a new destination, starting from wherever the
    /// piece is drawn now. Used when a piece is told to move again while it is
    /// still travelling, so the visible motion never jumps and never queues.
    pub fn retarget(&mut self, to: Vec2, duration: f32) {
        let lift = self.lift();
        self.from = self.position();
        self.start_lift = lift;
        self.to = to;
        self.elapsed = 0.0;
        self.duration = duration;
        self.status = VisualTransactionStatus::Uncommitted;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tx() -> MoveTransaction {
        MoveTransaction::new(
            Vec2::new(0.0, 1.0),
            Vec2::new(0.0, 3.0),
            0.4,
            MotionStyle::GAMEPLAY,
        )
    }

    #[test]
    fn commits_once_at_midpoint_and_completes_once() {
        let mut t = tx();
        let mut commits = 0;
        let mut completions = 0;
        for _ in 0..100 {
            let crossed = t.advance(0.01);
            commits += crossed.committed as u32;
            completions += crossed.completed as u32;
        }
        assert_eq!(commits, 1);
        assert_eq!(completions, 1);
        assert_eq!(t.status, VisualTransactionStatus::Completed);
        assert_eq!(t.position(), Vec2::new(0.0, 3.0));
    }

    #[test]
    fn a_frame_that_skips_the_midpoint_still_commits() {
        let mut t = tx();
        let first = t.advance(0.1);
        assert_eq!(first, Crossed::default());
        let second = t.advance(0.15);
        assert_eq!(
            second,
            Crossed {
                committed: true,
                completed: false
            }
        );
        let third = t.advance(0.01);
        assert_eq!(third, Crossed::default());
    }

    #[test]
    fn a_stall_commits_and_completes_in_the_same_advance_without_overshoot() {
        let mut t = tx();
        let crossed = t.advance(30.0);
        assert!(crossed.committed && crossed.completed);
        assert_eq!(t.position(), Vec2::new(0.0, 3.0));
        assert_eq!(t.lift(), 0.0);
        assert_eq!(t.advance(1.0), Crossed::default());
    }

    #[test]
    fn invalid_deltas_do_not_advance_time() {
        let mut t = tx();
        for delta in [f32::NAN, f32::INFINITY, -1.0, 0.0] {
            t.advance(delta);
        }
        assert_eq!(t.elapsed, 0.0);
        assert_eq!(t.status, VisualTransactionStatus::Uncommitted);
    }

    #[test]
    fn zero_duration_completes_on_first_advance() {
        let mut t = MoveTransaction::new(Vec2::ZERO, Vec2::ONE, 0.0, MotionStyle::GAMEPLAY);
        assert!(t.advance(0.0).completed);
        assert_eq!(t.position(), Vec2::ONE);
    }

    #[test]
    fn retarget_continues_from_the_drawn_position() {
        let mut t = tx();
        t.advance(0.2);
        let drawn = t.position();
        let lift = t.lift();
        t.retarget(Vec2::new(4.0, 3.0), 0.4);
        assert_eq!(t.from, drawn);
        assert_eq!(t.elapsed, 0.0);
        assert_eq!(t.position(), drawn);
        assert_eq!(t.lift(), lift);
        assert_eq!(t.status, VisualTransactionStatus::Uncommitted);
    }

    #[test]
    fn frame_rate_does_not_change_the_final_state() {
        for fps in [30.0_f32, 60.0, 144.0] {
            let mut t = tx();
            let mut frames = 0;
            while !t.advance(1.0 / fps).completed {
                frames += 1;
                assert!(frames < 1000);
            }
            assert_eq!(t.position(), Vec2::new(0.0, 3.0));
        }
        let mut irregular = tx();
        for delta in [0.001, 0.2, 0.0005, 0.35] {
            irregular.advance(delta);
        }
        assert!(irregular.is_complete());
    }
}
