//! Animation components + systems for mini pieces.

use bevy::prelude::*;

use super::board::BOARD_HALF;
use super::pieces::MiniPiece;

/// Smoothed arc motion from one square to another.
#[derive(Component)]
pub struct MiniMoveAnimation {
    pub start: Vec3,
    pub end: Vec3,
    pub elapsed: f32,
    pub duration: f32,
}

/// Shrink-to-nothing fade-out for captured pieces.
#[derive(Component)]
pub struct MiniFadeOut {
    pub timer: Timer,
    pub initial_scale: f32,
}

pub fn animate_moves(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Transform, &mut MiniMoveAnimation)>,
) {
    for (entity, mut transform, mut anim) in q.iter_mut() {
        anim.elapsed += time.delta_secs();
        let t = (anim.elapsed / anim.duration.max(0.0001)).clamp(0.0, 1.0);
        let smooth_t = t * t * (3.0 - 2.0 * t);

        // Gentle arc lift so pieces glide rather than slide.
        let arc_h = 0.22;
        let arc = (std::f32::consts::PI * t).sin() * arc_h;

        let lerped = anim.start.lerp(anim.end, smooth_t);
        transform.translation = Vec3::new(lerped.x, lerped.y + arc, lerped.z);

        if t >= 1.0 {
            transform.translation = anim.end;
            commands.entity(entity).remove::<MiniMoveAnimation>();
        }
    }
}

pub fn animate_captures(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Transform, &mut MiniFadeOut)>,
) {
    for (entity, mut transform, mut fade) in q.iter_mut() {
        fade.timer.tick(time.delta());
        let progress = fade.timer.fraction();
        let scale = fade.initial_scale * (1.0 - progress * progress);
        transform.scale = Vec3::splat(scale.max(0.0));
        if fade.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// Tiny idle hover so stationary pieces don't look frozen.
/// Respects the `MiniMoveAnimation` marker so moving pieces aren't perturbed.
pub fn idle_float(
    time: Res<Time>,
    mut q: Query<(&MiniPiece, &mut Transform), Without<MiniMoveAnimation>>,
) {
    let _ = BOARD_HALF; // keep import resolution tidy without functional impact
    let t = time.elapsed_secs();
    for (piece, mut transform) in q.iter_mut() {
        let phase = (piece.file as f32 * 1.3 + piece.rank as f32 * 0.9)
            % std::f32::consts::TAU;
        let float_y = (t * 0.5 + phase).sin() * 0.012;
        transform.translation.y = float_y;
    }
}
