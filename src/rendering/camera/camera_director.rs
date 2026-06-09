//! Camera Director — automated camera moves for chess-shorts cinematics.
//!
//! On blunder/brilliant/checkmate events the director smoothly zooms the
//! camera toward the destination square and then returns it to the overview.

use crate::core::GameMode;
use crate::game::replay_shorts::{BlunderFlash, BrilliantGlow, CheckmateFlash};
use crate::game::systems::camera::CameraController;
use crate::multiplayer::traits::MessageReader;
use bevy::prelude::*;

// ─────────────────────────────────────────────────────────────────────────────
// Resource
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum CameraDirectorMode {
    Free,
    ZoomIn {
        origin: Transform,
        target: Transform,
        elapsed: f32,
        duration: f32,
        hold_after: f32,
    },
    Hold {
        return_to: Transform,
        elapsed: f32,
        hold_duration: f32,
    },
    ZoomOut {
        from: Transform,
        target: Transform,
        elapsed: f32,
        duration: f32,
    },
}

impl Default for CameraDirectorMode {
    fn default() -> Self {
        CameraDirectorMode::Free
    }
}

#[derive(Resource, Default)]
pub struct CameraDirector {
    pub mode: CameraDirectorMode,
    /// Target world position to zoom toward (board square centre)
    pub target_sq: Option<Vec3>,
}

// ─────────────────────────────────────────────────────────────────────────────
// System
// ─────────────────────────────────────────────────────────────────────────────

/// Drives the camera toward/away from key board squares on cinematic events.
pub fn camera_director_system(
    time: Res<Time>,
    game_mode: Res<GameMode>,
    mut director: ResMut<CameraDirector>,
    mut blunder_ev: MessageReader<BlunderFlash>,
    mut brilliant_ev: MessageReader<BrilliantGlow>,
    mut checkmate_ev: MessageReader<CheckmateFlash>,
    mut cam_q: Query<&mut Transform, (With<Camera3d>, With<CameraController>)>,
) {
    if *game_mode != GameMode::PgnReplay { return; }

    let dt = time.delta_secs();
    let Ok(mut cam_tf) = cam_q.single_mut() else { return };

    // Consume events and kick off a zoom sequence
    let blunder = blunder_ev.read().next().is_some();
    let brilliant = brilliant_ev.read().next().is_some();
    let checkmate = checkmate_ev.read().next().is_some();

    if blunder || brilliant || checkmate {
        if let CameraDirectorMode::Free = director.mode {
            // Pick zoom duration / hold based on event type
            let (zoom_dur, hold_dur) = if checkmate {
                (1.2, 3.0)
            } else if brilliant {
                (0.6, 1.2)
            } else {
                (0.5, 1.0)
            };

            let target_world = director.target_sq.unwrap_or(Vec3::new(3.5, 0.0, 3.5));
            let origin = *cam_tf;
            // Zoom target: move 35% of the way toward (target + a bit of elevation)
            let zoom_pos = cam_tf.translation.lerp(target_world + Vec3::Y * 1.5, 0.35);
            let zoom_target = Transform::from_translation(zoom_pos)
                .looking_at(target_world, Vec3::Y);

            director.mode = CameraDirectorMode::ZoomIn {
                origin,
                target: zoom_target,
                elapsed: 0.0,
                duration: zoom_dur,
                hold_after: hold_dur,
            };
        }
    }

    // Drive state machine
    let new_mode = match &mut director.mode {
        CameraDirectorMode::Free => None,

        CameraDirectorMode::ZoomIn { origin, target, elapsed, duration, hold_after } => {
            *elapsed += dt;
            let t = smooth_step((*elapsed / *duration).min(1.0));
            cam_tf.translation = origin.translation.lerp(target.translation, t);
            // slerp rotation
            cam_tf.rotation = origin.rotation.slerp(target.rotation, t);
            if *elapsed >= *duration {
                Some(CameraDirectorMode::Hold {
                    return_to: *origin,
                    elapsed: 0.0,
                    hold_duration: *hold_after,
                })
            } else {
                None
            }
        }

        CameraDirectorMode::Hold { return_to, elapsed, hold_duration } => {
            *elapsed += dt;
            if *elapsed >= *hold_duration {
                let from = *cam_tf;
                Some(CameraDirectorMode::ZoomOut {
                    from,
                    target: *return_to,
                    elapsed: 0.0,
                    duration: 0.8,
                })
            } else {
                None
            }
        }

        CameraDirectorMode::ZoomOut { from, target, elapsed, duration } => {
            *elapsed += dt;
            let t = smooth_step((*elapsed / *duration).min(1.0));
            cam_tf.translation = from.translation.lerp(target.translation, t);
            cam_tf.rotation = from.rotation.slerp(target.rotation, t);
            if *elapsed >= *duration {
                Some(CameraDirectorMode::Free)
            } else {
                None
            }
        }
    };

    if let Some(m) = new_mode {
        director.mode = m;
    }
}

/// Smooth-step cubic ease-in-out.
fn smooth_step(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}
