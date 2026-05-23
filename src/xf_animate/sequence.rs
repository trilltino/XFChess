//! Sequence playback: steps through the encoded game and schedules animations.

use bevy::prelude::*;

use super::animation::{MiniFadeOut, MiniMoveAnimation};
use super::board::square_world;
use super::games::IMMORTAL_ZUGZWANG;
use super::pieces::{spawn_starting_position, MiniAssets, MiniPiece};
use crate::rendering::pieces::PieceType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveKind {
    Normal,
    Capture,
    CastleKingside,
    CastleQueenside,
    EnPassant,
    Promote(PieceType),
}

#[derive(Debug, Clone, Copy)]
pub struct MoveStep {
    pub from: (u8, u8),
    pub to: (u8, u8),
    pub kind: MoveKind,
}

#[derive(Resource)]
pub struct SequencePlayback {
    pub index: usize,
    pub move_timer: Timer,
    pub end_pause: Option<Timer>,
}

impl Default for SequencePlayback {
    fn default() -> Self {
        Self {
            index: 0,
            move_timer: Timer::from_seconds(2.75, TimerMode::Repeating),
            end_pause: None,
        }
    }
}

pub fn run_sequence(
    time: Res<Time>,
    mut playback: ResMut<SequencePlayback>,
    mut commands: Commands,
    mut pieces: Query<(Entity, &mut MiniPiece, &Transform), Without<MiniMoveAnimation>>,
) {
    if playback.end_pause.is_some() {
        return;
    }
    if playback.index >= IMMORTAL_ZUGZWANG.len() {
        return;
    }
    playback.move_timer.tick(time.delta());
    if !playback.move_timer.just_finished() {
        return;
    }

    let step = IMMORTAL_ZUGZWANG[playback.index];
    apply_step(&mut commands, &mut pieces, step);

    playback.index += 1;
    if playback.index >= IMMORTAL_ZUGZWANG.len() {
        playback.end_pause = Some(Timer::from_seconds(4.0, TimerMode::Once));
    }
}

fn apply_step(
    commands: &mut Commands,
    pieces: &mut Query<(Entity, &mut MiniPiece, &Transform), Without<MiniMoveAnimation>>,
    step: MoveStep,
) {
    let (from_file, from_rank) = step.from;
    let (to_file, to_rank) = step.to;

    let mut mover: Option<Entity> = None;
    let mut capture_target: Option<(Entity, f32)> = None;
    let mut rook_target: Option<(Entity, u8)> = None;

    for (entity, piece, transform) in pieces.iter() {
        if piece.file == from_file && piece.rank == from_rank {
            mover = Some(entity);
        }
        match step.kind {
            MoveKind::Capture | MoveKind::Promote(_) => {
                if piece.file == to_file && piece.rank == to_rank {
                    capture_target = Some((entity, transform.scale.x));
                }
            }
            MoveKind::EnPassant => {
                if piece.file == to_file && piece.rank == from_rank {
                    capture_target = Some((entity, transform.scale.x));
                }
            }
            MoveKind::CastleKingside => {
                if piece.file == 7 && piece.rank == to_rank && piece.kind == PieceType::Rook {
                    rook_target = Some((entity, 5));
                }
            }
            MoveKind::CastleQueenside => {
                if piece.file == 0 && piece.rank == to_rank && piece.kind == PieceType::Rook {
                    rook_target = Some((entity, 3));
                }
            }
            MoveKind::Normal => {}
        }
    }

    if let Some((entity, initial_scale)) = capture_target {
        commands.entity(entity).insert(MiniFadeOut {
            timer: Timer::from_seconds(0.45, TimerMode::Once),
            initial_scale,
        });
    }

    let Some(mover_entity) = mover else {
        warn!(
            "[XF_ANIMATE] no piece at ({},{}) for step {:?}",
            from_file, from_rank, step
        );
        return;
    };

    if let Ok((_, mut piece, transform)) = pieces.get_mut(mover_entity) {
        let start = transform.translation;
        let mut end = square_world(to_file, to_rank);
        end.y = start.y;
        commands.entity(mover_entity).insert(MiniMoveAnimation {
            start,
            end,
            elapsed: 0.0,
            duration: 1.4,
        });
        piece.file = to_file;
        piece.rank = to_rank;
    }

    if let Some((rook_entity, new_file)) = rook_target {
        if let Ok((_, mut piece, transform)) = pieces.get_mut(rook_entity) {
            let start = transform.translation;
            let mut end = square_world(new_file, piece.rank);
            end.y = start.y;
            commands.entity(rook_entity).insert(MiniMoveAnimation {
                start,
                end,
                elapsed: 0.0,
                duration: 1.1,
            });
            piece.file = new_file;
        }
    }
}

/// Once the trailing pause elapses, despawn remaining pieces and start over.
pub fn restart_when_complete(
    time: Res<Time>,
    mut playback: ResMut<SequencePlayback>,
    mut commands: Commands,
    pieces: Query<Entity, With<MiniPiece>>,
    assets: Option<Res<MiniAssets>>,
) {
    let Some(pause) = playback.end_pause.as_mut() else {
        return;
    };
    pause.tick(time.delta());
    if !pause.just_finished() {
        return;
    }

    for entity in pieces.iter() {
        commands.entity(entity).despawn();
    }
    if let Some(assets) = assets {
        spawn_starting_position(&mut commands, &assets);
    }
    *playback = SequencePlayback::default();
}
