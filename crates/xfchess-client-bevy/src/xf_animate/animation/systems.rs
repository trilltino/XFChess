use bevy::prelude::*;

use super::components::{MiniFadeOut, MiniMoveAnimation};
use crate::motion::MotionStyle;
use crate::xf_animate::pieces::MiniPiece;

pub fn animate_moves(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Transform, &mut MiniMoveAnimation)>,
) {
    for (entity, mut transform, mut anim) in q.iter_mut() {
        anim.elapsed += time.delta_secs();
        let sample = MotionStyle::MINI.sample(anim.elapsed, anim.duration, 0.0);
        transform.translation = sample.position3(anim.start, anim.end);
        if sample.complete {
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

pub fn idle_float(
    time: Res<Time>,
    mut q: Query<(&MiniPiece, &mut Transform), Without<MiniMoveAnimation>>,
) {
    let t = time.elapsed_secs();
    for (piece, mut transform) in q.iter_mut() {
        let phase = (piece.file as f32 * 1.3 + piece.rank as f32 * 0.9) % std::f32::consts::TAU;
        let float_y = (t * 0.5 + phase).sin() * 0.012;
        transform.translation.y = float_y;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learn_board_moves_use_the_shared_sampler_and_land_exactly(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.add_systems(Update, animate_moves);
        let (start, end) = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 2.0));
        let piece = app
            .world_mut()
            .spawn((
                Transform::from_translation(start),
                MiniMoveAnimation {
                    start,
                    end,
                    elapsed: 0.0,
                    duration: 1.4,
                },
            ))
            .id();

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(0.7));
        app.update();
        let mid = app
            .world()
            .get::<Transform>(piece)
            .ok_or("missing transform")?
            .translation;
        assert_eq!(
            mid,
            MotionStyle::MINI
                .sample(0.7, 1.4, 0.0)
                .position3(start, end)
        );
        assert!(mid.y > 0.0);

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(5.0));
        app.update();
        app.update();
        let landed = app
            .world()
            .get::<Transform>(piece)
            .ok_or("missing transform")?
            .translation;
        assert_eq!(landed, end);
        assert!(app.world().get::<MiniMoveAnimation>(piece).is_none());
        Ok(())
    }
}
