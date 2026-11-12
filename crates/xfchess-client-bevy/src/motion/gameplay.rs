//! Live-game piece movement: one [`PieceMotion`] per moving piece entity, advanced
//! once per frame and read by both the 3D and the 2D board.
//!
//! The authoritative [`Piece`] coordinates are already at the destination when a
//! motion starts. A motion only decides where the piece is *drawn*; nothing here
//! touches the engine, turn, clocks, history or network.

use super::curve::{MotionStyle, GAMEPLAY_MOVE_SECONDS};
use super::timeline::MoveTransaction;
use crate::game::components::FadingCapture;
use crate::rendering::pieces::{Piece, PieceType, PIECE_ON_BOARD_Y};
use bevy::ecs::system::Command;
use bevy::prelude::*;

/// Identifies one board incarnation. Resyncs, resets and new games bump it so a
/// motion or premove created for an earlier board can never act on a later one.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BoardEpoch(pub u64);

/// Local presentation preferences. Neither value participates in chess rules
/// or in the multiplayer protocol.
#[derive(Resource, Debug, Clone, Copy)]
pub struct MotionSettings {
    pub enabled: bool,
    pub duration: f32,
}

impl Default for MotionSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            duration: GAMEPLAY_MOVE_SECONDS,
        }
    }
}

impl BoardEpoch {
    pub fn bump(&mut self) {
        self.0 = self.0.wrapping_add(1);
    }
}

/// A piece travelling visually to the square its [`Piece`] already occupies.
#[derive(Component, Debug, Clone)]
pub struct PieceMotion {
    pub tx: MoveTransaction,
    pub epoch: u64,
    /// Set when the move promotes: the type drawn until the midpoint.
    pub promoted_from: Option<PieceType>,
    /// Commands are flushed before Visual; don't spend the preceding frame's
    /// delta on a transaction that was only just created.
    pub started_at: std::time::Duration,
}

impl PieceMotion {
    /// The type to draw: the pre-promotion type until the midpoint, then the
    /// piece's real (promoted) type.
    pub fn drawn_type(&self, current: PieceType) -> PieceType {
        match self.promoted_from {
            Some(from) if !self.tx.sample().committed => from,
            _ => current,
        }
    }
}

pub fn square_to_board(square: (u8, u8)) -> Vec2 {
    Vec2::new(square.0 as f32, square.1 as f32)
}

/// The 3D scene mirrors files: world X is `7 - file`, world Z is the rank.
pub fn board_to_world(position: Vec2) -> Vec3 {
    Vec3::new(7.0 - position.x, PIECE_ON_BOARD_Y, position.y)
}

/// Retires every motion and the queued premove: the board they were made for
/// is no longer trusted (resync, FEN mismatch). Queue it as a command after the
/// motions to retire so it runs after they are stamped; pieces then snap to
/// their authoritative squares.
pub fn retire_board_visuals(world: &mut World) {
    let old_epoch = world
        .get_resource::<BoardEpoch>()
        .copied()
        .unwrap_or_default()
        .0;
    if let Some(mut epoch) = world.get_resource_mut::<BoardEpoch>() {
        epoch.bump();
    }
    let new_epoch = world
        .get_resource::<BoardEpoch>()
        .copied()
        .unwrap_or_default()
        .0;
    if let Some(mut checkpoint) =
        world.get_resource_mut::<crate::game::systems::board_snapshot::VerifiedBoardPosition>()
    {
        if checkpoint.epoch == old_epoch {
            checkpoint.epoch = new_epoch;
        }
    }
    if let Some(mut premove) = world.get_resource_mut::<crate::game::resources::premove::Premove>()
    {
        premove.clear();
    }
    let mut captures = world.query_filtered::<Entity, With<FadingCapture>>();
    let captures: Vec<_> = captures.iter(world).collect();
    for entity in captures {
        world.despawn(entity);
    }
}

/// Starts (or retargets) the visual motion of `entity` between two squares.
/// Queued as a command so it reads the current [`BoardEpoch`] when applied.
pub struct StartPieceMotion {
    pub entity: Entity,
    pub from: (u8, u8),
    pub to: (u8, u8),
    pub duration: f32,
    pub promoted_from: Option<PieceType>,
    use_gameplay_settings: bool,
}

impl StartPieceMotion {
    pub fn gameplay(entity: Entity, from: (u8, u8), to: (u8, u8)) -> Self {
        Self {
            entity,
            from,
            to,
            duration: GAMEPLAY_MOVE_SECONDS,
            promoted_from: None,
            use_gameplay_settings: true,
        }
    }

    pub fn with_duration(mut self, duration: f32) -> Self {
        self.duration = duration;
        self.use_gameplay_settings = false;
        self
    }

    pub fn promoting_from(mut self, from: Option<PieceType>) -> Self {
        self.promoted_from = from;
        self
    }
}

impl Command for StartPieceMotion {
    type Out = ();

    fn apply(mut self, world: &mut World) {
        let settings = world
            .get_resource::<MotionSettings>()
            .copied()
            .unwrap_or_default();
        if self.use_gameplay_settings {
            self.duration = settings.duration;
        }
        let epoch = world
            .get_resource::<BoardEpoch>()
            .copied()
            .unwrap_or_default()
            .0;
        let started_at = world
            .get_resource::<Time>()
            .map(|t| t.elapsed())
            .unwrap_or_default();
        let Ok(mut entity) = world.get_entity_mut(self.entity) else {
            return;
        };
        if entity.contains::<FadingCapture>()
            || entity.get::<Piece>().is_none_or(|p| p.x >= 8 || p.y >= 8)
        {
            return;
        }
        if !settings.enabled {
            entity.remove::<PieceMotion>();
            return;
        }
        let to = square_to_board(self.to);
        if let Some(mut motion) = entity.get_mut::<PieceMotion>() {
            if motion.epoch == epoch {
                // Same piece told to move again mid-flight: continue from where
                // it is drawn rather than queueing behind the old move.
                if motion.tx.sample().committed {
                    motion.promoted_from = None;
                }
                motion.tx.retarget(to, self.duration);
                motion.started_at = started_at;
                if self.promoted_from.is_some() {
                    motion.promoted_from = self.promoted_from;
                }
                return;
            }
        }
        entity.insert(PieceMotion {
            tx: MoveTransaction::new(
                square_to_board(self.from),
                to,
                self.duration,
                MotionStyle::GAMEPLAY,
            ),
            epoch,
            promoted_from: self.promoted_from,
            started_at,
        });
    }
}

/// Ticks every active motion once per frame, drops finished ones and drops any
/// that belong to an earlier board.
pub fn advance_piece_motions(
    time: Res<Time>,
    epoch: Res<BoardEpoch>,
    mut commands: Commands,
    mut motions: Query<(Entity, &mut PieceMotion)>,
    settings: Option<Res<MotionSettings>>,
) {
    let delta = time.delta_secs();
    for (entity, mut motion) in &mut motions {
        if motion.epoch != epoch.0 || settings.as_ref().is_some_and(|settings| !settings.enabled) {
            commands.entity(entity).remove::<PieceMotion>();
            continue;
        }
        let delta = if motion.started_at == time.elapsed() {
            0.0
        } else {
            delta
        };
        if motion.tx.advance(delta).completed {
            commands.entity(entity).remove::<PieceMotion>();
        }
    }
}

/// The 3D adapter: the only writer of a moving piece's translation. Pieces
/// without a motion rest on their authoritative square.
pub fn present_piece_motion_3d(
    mut pieces: Query<(&mut Transform, &Piece, Option<&PieceMotion>), Without<FadingCapture>>,
) {
    for (mut transform, piece, motion) in &mut pieces {
        if let Some(motion) = motion {
            let sample = motion.tx.sample();
            let ground = board_to_world(sample.position2(motion.tx.from, motion.tx.to));
            transform.translation = ground + Vec3::Y * sample.lift;
        } else {
            let rest = board_to_world(square_to_board((piece.x, piece.y)));
            if transform.translation != rest {
                transform.translation = rest;
            }
        }
    }
}

/// Keep child visuals in step with the type drawn by egui. Mesh replacement
/// does not despawn the logical entity, its picking observers or its timeline.
pub fn present_piece_types(
    meshes: Option<Res<crate::rendering::pieces::PieceMeshes>>,
    sprites: Option<Res<crate::rendering::pieces::PieceSpriteHandles>>,
    mut pieces: Query<(&Piece, Option<&PieceMotion>, &Children, &mut Transform)>,
    mut mesh_children: Query<
        &mut Mesh3d,
        (
            With<crate::rendering::pieces::Piece3DVisual>,
            Without<Piece>,
        ),
    >,
    mut sprite_children: Query<
        &mut Sprite,
        (
            With<crate::rendering::pieces::Piece2DVisual>,
            Without<Piece>,
        ),
    >,
) {
    let Some(meshes) = meshes else { return };
    for (piece, motion, children, mut transform) in &mut pieces {
        let kind = motion.map_or(piece.piece_type, |motion| {
            motion.drawn_type(piece.piece_type)
        });
        for child in children.iter() {
            if let Ok(mut mesh) = mesh_children.get_mut(child) {
                let target = meshes.get(kind, piece.color);
                if mesh.0 != target {
                    mesh.0 = target;
                    transform.rotation = match (kind, piece.color) {
                        (PieceType::Knight, crate::rendering::pieces::PieceColor::White) => {
                            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
                        }
                        (PieceType::Knight, _) => {
                            Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2)
                        }
                        (_, crate::rendering::pieces::PieceColor::White) => Quat::IDENTITY,
                        _ => Quat::from_rotation_y(std::f32::consts::PI),
                    };
                }
            }
            if let (Some(sprites), Ok(mut sprite)) = (&sprites, sprite_children.get_mut(child)) {
                let target = sprites.get(kind, piece.color);
                if sprite.image != target {
                    sprite.image = target;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rendering::pieces::PieceColor;
    use std::time::Duration;

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.init_resource::<BoardEpoch>();
        app.add_systems(
            Update,
            (advance_piece_motions, present_piece_motion_3d).chain(),
        );
        app
    }

    fn step(app: &mut App, seconds: f32) {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(seconds));
        app.update();
    }

    fn spawn_knight(app: &mut App, square: (u8, u8)) -> Entity {
        let rest = board_to_world(square_to_board(square));
        app.world_mut()
            .spawn((
                Piece::new(PieceColor::White, PieceType::Knight, square.0, square.1),
                Transform::from_translation(rest),
            ))
            .id()
    }

    fn start(app: &mut App, entity: Entity, from: (u8, u8), to: (u8, u8)) {
        StartPieceMotion::gameplay(entity, from, to).apply(app.world_mut());
    }

    fn move_authoritatively(app: &mut App, entity: Entity, to: (u8, u8)) {
        if let Some(mut piece) = app.world_mut().entity_mut(entity).get_mut::<Piece>() {
            piece.x = to.0;
            piece.y = to.1;
        }
    }

    fn translation(app: &App, entity: Entity) -> Option<Vec3> {
        app.world().get::<Transform>(entity).map(|t| t.translation)
    }

    #[test]
    fn piece_travels_from_origin_and_lands_exactly() -> Result<(), String> {
        let mut app = app();
        let knight = spawn_knight(&mut app, (6, 0));
        move_authoritatively(&mut app, knight, (5, 2));
        start(&mut app, knight, (6, 0), (5, 2));

        step(&mut app, 0.0);
        let first = translation(&app, knight).ok_or("missing transform")?;
        assert!((first - board_to_world(Vec2::new(6.0, 0.0))).length() < 1e-4);

        step(&mut app, 0.2);
        let mid = translation(&app, knight).ok_or("missing transform")?;
        assert!(mid.y > PIECE_ON_BOARD_Y);
        assert!(app.world().get::<PieceMotion>(knight).is_some());

        step(&mut app, 0.5);
        step(&mut app, 0.0);
        let end = translation(&app, knight).ok_or("missing transform")?;
        assert_eq!(end, board_to_world(Vec2::new(5.0, 2.0)));
        assert!(app.world().get::<PieceMotion>(knight).is_none());
        Ok(())
    }

    #[test]
    fn restarting_a_moving_piece_retargets_without_a_jump() -> Result<(), String> {
        let mut app = app();
        let knight = spawn_knight(&mut app, (6, 0));
        move_authoritatively(&mut app, knight, (5, 2));
        start(&mut app, knight, (6, 0), (5, 2));
        step(&mut app, 0.1);
        let drawn = app
            .world()
            .get::<PieceMotion>(knight)
            .ok_or("no motion")?
            .tx
            .position();
        move_authoritatively(&mut app, knight, (3, 3));
        start(&mut app, knight, (5, 2), (3, 3));
        let motion = app.world().get::<PieceMotion>(knight).ok_or("no motion")?;
        assert_eq!(motion.tx.from, drawn);
        assert_eq!(motion.tx.to, Vec2::new(3.0, 3.0));
        Ok(())
    }

    #[test]
    fn motion_from_an_earlier_board_is_dropped_and_the_piece_snaps() -> Result<(), String> {
        let mut app = app();
        let knight = spawn_knight(&mut app, (6, 0));
        move_authoritatively(&mut app, knight, (5, 2));
        start(&mut app, knight, (6, 0), (5, 2));
        step(&mut app, 0.1);
        app.world_mut().resource_mut::<BoardEpoch>().bump();
        step(&mut app, 0.0);
        step(&mut app, 0.0);
        assert!(app.world().get::<PieceMotion>(knight).is_none());
        let rest = translation(&app, knight).ok_or("missing transform")?;
        assert_eq!(rest, board_to_world(Vec2::new(5.0, 2.0)));
        Ok(())
    }

    #[test]
    fn two_pieces_animate_concurrently() -> Result<(), String> {
        let mut app = app();
        let king = spawn_knight(&mut app, (4, 0));
        let rook = spawn_knight(&mut app, (7, 0));
        move_authoritatively(&mut app, king, (6, 0));
        move_authoritatively(&mut app, rook, (5, 0));
        start(&mut app, king, (4, 0), (6, 0));
        start(&mut app, rook, (7, 0), (5, 0));
        step(&mut app, 0.1);
        assert!(app.world().get::<PieceMotion>(king).is_some());
        assert!(app.world().get::<PieceMotion>(rook).is_some());
        step(&mut app, 1.0);
        step(&mut app, 0.0);
        assert!(app.world().get::<PieceMotion>(king).is_none());
        assert!(app.world().get::<PieceMotion>(rook).is_none());
        assert_eq!(
            translation(&app, king).ok_or("missing transform")?,
            board_to_world(Vec2::new(6.0, 0.0))
        );
        Ok(())
    }

    #[test]
    fn promotion_draws_the_old_type_until_the_midpoint() -> Result<(), String> {
        let mut app = app();
        let pawn = spawn_knight(&mut app, (0, 6));
        StartPieceMotion::gameplay(pawn, (0, 6), (0, 7))
            .promoting_from(Some(PieceType::Pawn))
            .apply(app.world_mut());
        let before = app.world().get::<PieceMotion>(pawn).ok_or("no motion")?;
        assert_eq!(before.drawn_type(PieceType::Queen), PieceType::Pawn);
        step(&mut app, 0.3);
        let after = app.world().get::<PieceMotion>(pawn).ok_or("no motion")?;
        assert_eq!(after.drawn_type(PieceType::Queen), PieceType::Queen);
        Ok(())
    }

    #[test]
    fn promotion_mesh_and_egui_type_switch_together_without_replacing_the_piece(
    ) -> Result<(), String> {
        use crate::rendering::pieces::{Piece3DVisual, PieceMeshes};
        let mut app = app();
        let mut assets = Assets::<Mesh>::default();
        let pawn_mesh = assets.add(Cuboid::new(1.0, 1.0, 1.0));
        let queen_mesh = assets.add(Cuboid::new(1.0, 2.0, 1.0));
        app.insert_resource(PieceMeshes {
            white_pawn: pawn_mesh.clone(),
            white_queen: queen_mesh.clone(),
            white_king: Default::default(),
            white_rook: Default::default(),
            white_bishop: Default::default(),
            white_knight: Default::default(),
            black_pawn: Default::default(),
            black_queen: Default::default(),
            black_king: Default::default(),
            black_rook: Default::default(),
            black_bishop: Default::default(),
            black_knight: Default::default(),
        });
        app.add_systems(Update, present_piece_types.after(present_piece_motion_3d));
        let child = app
            .world_mut()
            .spawn((Mesh3d(pawn_mesh.clone()), Piece3DVisual))
            .id();
        let piece = app
            .world_mut()
            .spawn((
                Piece::new(PieceColor::White, PieceType::Queen, 0, 7),
                Transform::default(),
            ))
            .add_child(child)
            .id();
        StartPieceMotion::gameplay(piece, (0, 6), (0, 7))
            .promoting_from(Some(PieceType::Pawn))
            .apply(app.world_mut());
        step(&mut app, 0.0);
        assert_eq!(
            app.world().get::<Mesh3d>(child).map(|m| &m.0),
            Some(&pawn_mesh)
        );
        step(&mut app, 0.25);
        assert_eq!(
            app.world().get::<Mesh3d>(child).map(|m| &m.0),
            Some(&queen_mesh)
        );
        assert_eq!(
            app.world()
                .get::<PieceMotion>(piece)
                .ok_or("no timeline")?
                .drawn_type(PieceType::Queen),
            PieceType::Queen
        );
        assert_eq!(
            app.world()
                .get::<Piece>(piece)
                .ok_or("piece replaced")?
                .piece_type,
            PieceType::Queen
        );
        Ok(())
    }
}
