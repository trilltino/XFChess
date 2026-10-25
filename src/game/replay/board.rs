use super::*;

/// A forward ply updates existing entities. Seeks and incompatible boards fall
/// through to reconstruction; unrelated motion and capture tracks keep running.
pub fn replay_advance_pieces_system(
    mut commands: Commands,
    mut replay: ResMut<PgnReplayState>,
    mut pieces: Query<
        (Entity, &mut Piece, &mut HasMoved),
        Without<crate::game::components::FadingCapture>,
    >,
    mut captured: ResMut<crate::game::resources::CapturedPieces>,
) {
    if replay.board_ready || !replay.animate_next_advance || replay.engine_ply == 0 {
        return;
    }
    let prev = replay.prev_board;
    let curr = replay.engine.board;
    let entities: std::collections::HashMap<_, _> = pieces
        .iter()
        .filter(|(_, p, _)| p.x < 8 && p.y < 8)
        .map(|(e, p, _)| (p.y as usize * 8 + p.x as usize, (e, *p)))
        .collect();
    if !(0..64).all(|sq| match entities.get(&sq) {
        Some((_, p)) => {
            let id = ChessEngine::piece_type_to_id(p.piece_type)
                * if p.color == PieceColor::White { 1 } else { -1 };
            id == prev[sq]
        }
        None => prev[sq] == 0,
    }) {
        return;
    }
    let moves = replay_motion_squares(&prev, &curr);
    if moves.is_empty() {
        return;
    }
    let square = |sq: usize| ((sq % 8) as u8, (sq / 8) as u8);
    let duration = GAMEPLAY_MOVE_SECONDS / replay.slow_factor.max(0.05);
    let (from, _) = moves[0];
    let from_world = crate::motion::board_to_world(crate::motion::square_to_board(square(from)));
    for (&sq, &(entity, piece)) in &entities {
        if prev[sq] != curr[sq] && !moves.iter().any(|&(src, _)| src == sq) {
            let position =
                crate::motion::board_to_world(crate::motion::square_to_board(square(sq)));
            crate::game::systems::shared::apply_capture(
                &mut commands,
                &mut captured,
                None,
                crate::game::systems::shared::CapturedTarget {
                    entity,
                    piece_type: piece.piece_type,
                    color: piece.color,
                },
                position,
                position - from_world,
            );
            if let Ok((_, mut piece, _)) = pieces.get_mut(entity) {
                piece.x = u8::MAX;
                piece.y = u8::MAX;
            }
        }
    }
    for (src, dst) in moves {
        let (entity, previous) = entities[&src];
        if let Ok((_, mut piece, mut moved)) = pieces.get_mut(entity) {
            (piece.x, piece.y) = square(dst);
            piece.piece_type = engine_id_to_piece_type(curr[dst].abs());
            moved.moved = true;
            moved.move_count += 1;
        }
        commands.queue(
            StartPieceMotion::gameplay(entity, square(src), square(dst))
                .with_duration(duration)
                .promoting_from(
                    (previous.piece_type == PieceType::Pawn
                        && curr[dst].abs() != xfchess_rules::PAWN_ID)
                        .then_some(PieceType::Pawn),
                ),
        );
    }
    replay.animate_next_advance = false;
    replay.board_ready = true;
}

pub fn replay_spawn_pieces_system(
    mut commands: Commands,
    mut replay: ResMut<PgnReplayState>,
    mut engine: ResMut<ChessEngine>,
    asset_server: Res<AssetServer>,
    piece_meshes: Res<PieceMeshes>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut pieces_spawned: ResMut<PiecesSpawned>,
    existing_pieces: Query<Entity, With<Piece>>,
    sprite_handles: Option<Res<PieceSpriteHandles>>,
    mut epoch: ResMut<crate::motion::BoardEpoch>,
    mut premove: ResMut<crate::game::resources::premove::Premove>,
) {
    if replay.board_ready {
        return;
    }

    // Wait for meshes to load
    let meshes_to_check = piece_meshes.all_ids();
    for mesh_id in meshes_to_check.iter() {
        match asset_server.load_state(*mesh_id) {
            bevy::asset::LoadState::Loaded => {}
            _ => return,
        }
    }

    info!("[REPLAY] Spawning pieces from engine board");
    epoch.bump();
    premove.clear();

    for entity in existing_pieces.iter() {
        commands.entity(entity).despawn();
    }

    // Copy state we need before mutably borrowing replay later
    let animate = replay.animate_next_advance;
    let engine_ply = replay.engine_ply;
    let slow_factor = replay.slow_factor;
    let prev_board = replay.prev_board;
    let curr_board = replay.engine.board;

    // Spawn pieces from engine board; collect entity at each square for tween injection
    let mut entity_at_sq: std::collections::HashMap<usize, Entity> =
        std::collections::HashMap::new();

    for (sq, &piece_id) in curr_board.iter().enumerate() {
        if piece_id == 0 {
            continue;
        }
        let file = (sq % 8) as u8;
        let rank = (sq / 8) as u8;
        let color = if piece_id > 0 {
            PieceColor::White
        } else {
            PieceColor::Black
        };
        let Some(id) = piece_id.checked_abs().filter(|id| (1..=6).contains(id)) else {
            warn!("Invalid replay piece {piece_id} at square {sq}");
            continue;
        };
        let piece_type = engine_id_to_piece_type(id);

        let piece_material = if color == PieceColor::White {
            materials.add(crate::rendering::pieces::white_piece_material())
        } else {
            materials.add(crate::rendering::pieces::black_piece_material())
        };

        let entity = spawn_piece_at_replay(
            &mut commands,
            &piece_meshes,
            piece_material,
            color,
            piece_type,
            (file, rank),
            Vec3::ZERO,
            &sprite_handles,
        );
        entity_at_sq.insert(sq, entity);
    }

    // A single forward step draws the move through the shared motion timeline;
    // any other jump (seek, step back) just shows the rebuilt position.
    if animate && engine_ply > 0 {
        let duration = GAMEPLAY_MOVE_SECONDS / slow_factor.max(0.05);
        let square = |sq: usize| ((sq % 8) as u8, (sq / 8) as u8);
        for (src, dst) in replay_motion_squares(&prev_board, &curr_board) {
            if let Some(&entity) = entity_at_sq.get(&dst) {
                commands.queue(
                    StartPieceMotion::gameplay(entity, square(src), square(dst))
                        .with_duration(duration)
                        .promoting_from(
                            (prev_board[src].abs() == xfchess_rules::PAWN_ID
                                && curr_board[dst].abs() != xfchess_rules::PAWN_ID)
                                .then_some(PieceType::Pawn),
                        ),
                );
            }
        }
    }
    replay.animate_next_advance = false;

    // Sync engine to ECS
    engine.refresh_position();

    replay.board_ready = true;
    pieces_spawned.spawned = true;
    info!("[REPLAY] Pieces spawned successfully");
}

pub(super) fn engine_to_fen(game: &xfchess_rules::Game) -> String {
    game_to_fen(game)
}

pub(super) fn engine_id_to_piece_type(id: i8) -> PieceType {
    use xfchess_rules::{BISHOP_ID, KING_ID, KNIGHT_ID, PAWN_ID, QUEEN_ID, ROOK_ID};
    match id {
        PAWN_ID => PieceType::Pawn,
        KNIGHT_ID => PieceType::Knight,
        BISHOP_ID => PieceType::Bishop,
        ROOK_ID => PieceType::Rook,
        QUEEN_ID => PieceType::Queen,
        KING_ID => PieceType::King,
        _ => PieceType::Pawn,
    }
}

pub(super) fn replay_piece_rotation(piece_type: PieceType, color: PieceColor) -> Quat {
    match piece_type {
        PieceType::Knight => match color {
            PieceColor::White => Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            PieceColor::Black => Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2),
        },
        _ => match color {
            PieceColor::White => Quat::IDENTITY,
            PieceColor::Black => Quat::from_rotation_y(std::f32::consts::PI),
        },
    }
}

pub(super) fn spawn_piece_at_replay(
    commands: &mut Commands,
    meshes: &PieceMeshes,
    material: Handle<StandardMaterial>,
    color: PieceColor,
    piece_type: PieceType,
    position: (u8, u8),
    _visual_offset: Vec3,
    sprite_handles: &Option<Res<PieceSpriteHandles>>,
) -> Entity {
    let (file, rank) = position;
    let world_pos = Vec3::new(7.0 - file as f32, PIECE_ON_BOARD_Y, rank as f32);

    let mesh = meshes.get(piece_type, color);
    let rotation = replay_piece_rotation(piece_type, color);
    let name = format!("{:?} {:?} at ({},{})", color, piece_type, file, rank);

    commands
        .spawn((
            Piece {
                piece_type,
                color,
                x: file,
                y: rank,
            },
            HasMoved::default(),
            Transform::from_translation(world_pos).with_rotation(rotation),
            Visibility::default(),
            Name::new(name),
            game_scene(),
            bevy::camera::visibility::RenderLayers::layer(
                crate::game::systems::camera::BOARD_LAYER,
            ),
        ))
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::default(),
                Piece3DVisual,
                bevy::camera::visibility::RenderLayers::layer(
                    crate::game::systems::camera::BOARD_LAYER,
                ),
            ));

            if let Some(handles) = sprite_handles {
                let sprite = handles.get(piece_type, color);
                parent.spawn((
                    Sprite::from_image(sprite),
                    Transform::from_xyz(0.0, 0.1, 0.0)
                        .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
                        .with_scale(Vec3::splat(0.002)),
                    Piece2DVisual,
                    Visibility::Hidden,
                ));
            }
        })
        .id()
}

/// The piece moves between two replay boards as `(from, to)` square indices:
/// the mover first, then the rook when the move was castling. Empty when the
/// boards differ by anything other than one move.
pub(super) fn replay_motion_squares(prev: &[i8; 64], curr: &[i8; 64]) -> Vec<(usize, usize)> {
    // Prefer a moved king: the rook's destination sorts before the king's
    // when castling kingside. Never pair an en-passant victim with the mover.
    let destinations: Vec<_> = (0..64)
        .filter(|&sq| {
            curr[sq] != 0
                && prev[sq] != curr[sq]
                && (prev[sq] == 0 || prev[sq].signum() != curr[sq].signum())
        })
        .collect();
    let dst = destinations
        .iter()
        .copied()
        .find(|&sq| curr[sq].abs() == xfchess_rules::KING_ID)
        .or_else(|| destinations.first().copied());
    let Some(dst) = dst else { return Vec::new() };
    let src = (0..64).find(|&sq| {
        prev[sq] != 0
            && curr[sq] == 0
            && prev[sq].signum() == curr[dst].signum()
            && (prev[sq] == curr[dst] || prev[sq].abs() == xfchess_rules::PAWN_ID)
    });
    let Some(src) = src else { return Vec::new() };
    let mut moves = vec![(src, dst)];
    if curr[dst].abs() == xfchess_rules::KING_ID
        && src / 8 == dst / 8
        && (src % 8).abs_diff(dst % 8) == 2
    {
        let rank = dst / 8;
        moves.push(if dst % 8 > src % 8 {
            (rank * 8 + 7, rank * 8 + 5)
        } else {
            (rank * 8, rank * 8 + 3)
        });
    }
    // Only a single chess move may be animated. Arbitrary reconstruction snaps.
    let explained = (0..64).all(|sq| {
        prev[sq] == curr[sq]
            || moves.iter().any(|&(from, to)| sq == from || sq == to)
            || (prev[sq].abs() == xfchess_rules::PAWN_ID
                && curr[sq] == 0
                && prev[sq].signum() != curr[dst].signum()
                && sq / 8 == src / 8
                && sq % 8 == dst % 8
                && prev[src].abs() == xfchess_rules::PAWN_ID)
    });
    if explained {
        moves
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xfchess_rules::{KING_ID, PAWN_ID, QUEEN_ID, ROOK_ID};

    fn board(pieces: &[(usize, i8)]) -> [i8; 64] {
        let mut board = [0i8; 64];
        for &(sq, id) in pieces {
            board[sq] = id;
        }
        board
    }

    fn replay_app(fen: &str) -> Result<App, String> {
        use crate::game::resources::{
            premove::Premove, CapturedPieces, CurrentTurn, PendingPromotion, PendingTurnAdvance,
            Selection,
        };
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<ChessEngine>()
            .init_resource::<crate::motion::BoardEpoch>()
            .init_resource::<CapturedPieces>()
            .init_resource::<CurrentTurn>()
            .init_resource::<PendingPromotion>()
            .init_resource::<PendingTurnAdvance>()
            .init_resource::<Selection>()
            .init_resource::<Premove>();
        app.insert_resource(crate::game::systems::board_snapshot::PendingBoardSnapshot(
            Some(fen.into()),
        ));
        app.insert_resource(PgnReplayState {
            engine: Box::new(xfchess_rules::Game::from_fen(fen)?),
            board_ready: true,
            slow_factor: 0.35,
            ..Default::default()
        });
        app.add_message::<crate::game::events::MoveMadeEvent>();
        app.add_systems(
            Update,
            (
                crate::game::systems::board_snapshot::apply_board_snapshot,
                super::super::replay_sync_engine_system,
                replay_advance_pieces_system,
                crate::motion::advance_piece_motions,
                crate::motion::present_piece_motion_3d,
                crate::game::systems::visual::animate_capture_fade,
            )
                .chain(),
        );
        app.update();
        Ok(app)
    }

    fn advance(app: &mut App, uci: &str) -> Result<(), String> {
        let mut replay = app.world_mut().resource_mut::<PgnReplayState>();
        replay.prev_board = replay.engine.board;
        replay.engine.play_uci(uci)?;
        replay.engine_ply += 1;
        replay.board_ready = false;
        replay.animate_next_advance = true;
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::ZERO);
        app.update();
        Ok(())
    }

    fn entity_at(app: &mut App, square: (u8, u8)) -> Option<Entity> {
        app.world_mut()
            .query::<(Entity, &Piece)>()
            .iter(app.world())
            .find(|(_, piece)| (piece.x, piece.y) == square)
            .map(|(entity, _)| entity)
    }

    #[test]
    fn fast_forward_preserves_entities_and_overlapping_tracks_at_each_frame_rate(
    ) -> Result<(), String> {
        for fps in [30, 60, 144] {
            let mut app = replay_app(xfchess_rules::STARTING_FEN)?;
            let pawn = entity_at(&mut app, (4, 1)).ok_or("missing pawn")?;
            let epoch = app.world().resource::<crate::motion::BoardEpoch>().0;
            advance(&mut app, "e2e4")?;
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_secs_f32(0.1));
            app.update();
            let before = app
                .world()
                .get::<crate::motion::PieceMotion>(pawn)
                .ok_or("missing motion")?
                .tx;
            advance(&mut app, "e7e5")?;
            assert_eq!(entity_at(&mut app, (4, 3)), Some(pawn));
            assert_eq!(
                app.world()
                    .get::<crate::motion::PieceMotion>(pawn)
                    .ok_or("lost previous track")?
                    .tx,
                before
            );
            advance(&mut app, "g1f3")?;
            assert_eq!(
                app.world_mut()
                    .query::<&crate::motion::PieceMotion>()
                    .iter(app.world())
                    .count(),
                3
            );
            assert_eq!(app.world().resource::<crate::motion::BoardEpoch>().0, epoch);
            for _ in 0..fps * 2 {
                app.world_mut()
                    .resource_mut::<Time>()
                    .advance_by(std::time::Duration::from_secs_f32(1.0 / fps as f32));
                app.update();
            }
            assert_eq!(
                app.world_mut()
                    .query::<&crate::motion::PieceMotion>()
                    .iter(app.world())
                    .count(),
                0
            );
            assert!(app
                .world()
                .resource::<Messages<crate::game::events::MoveMadeEvent>>()
                .is_empty());
        }
        Ok(())
    }

    #[test]
    fn forward_special_moves_keep_the_mover_and_clean_up_captures() -> Result<(), String> {
        for (fen, uci, from, to, tracks, victim) in [
            (
                "4k3/8/8/3p4/4P3/8/8/4K3 w - - 0 1",
                "e4d5",
                (4, 3),
                (3, 4),
                1,
                Some((3, 4)),
            ),
            (
                "4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1",
                "e5d6",
                (4, 4),
                (3, 5),
                1,
                Some((3, 4)),
            ),
            (
                "4k3/8/8/8/8/8/8/R3K2R w KQ - 0 1",
                "e1g1",
                (4, 0),
                (6, 0),
                2,
                None,
            ),
            (
                "1r2k3/P7/8/8/8/8/8/4K3 w - - 0 1",
                "a7b8n",
                (0, 6),
                (1, 7),
                1,
                Some((1, 7)),
            ),
        ] {
            let mut app = replay_app(fen)?;
            let mover = entity_at(&mut app, from).ok_or("missing mover")?;
            let captured = victim.and_then(|square| entity_at(&mut app, square));
            advance(&mut app, uci)?;
            assert_eq!(entity_at(&mut app, to), Some(mover));
            assert_eq!(
                app.world_mut()
                    .query::<&crate::motion::PieceMotion>()
                    .iter(app.world())
                    .count(),
                tracks
            );
            if let Some(victim) = captured {
                assert!(app
                    .world()
                    .get::<crate::game::components::FadingCapture>(victim)
                    .is_some());
            }
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_secs(2));
            app.update();
            if let Some(victim) = captured {
                assert!(app.world().get_entity(victim).is_err());
            }
            assert!(app
                .world()
                .resource::<Messages<crate::game::events::MoveMadeEvent>>()
                .is_empty());
        }
        Ok(())
    }

    #[test]
    fn a_normal_move_is_one_pair() {
        let prev = board(&[(12, PAWN_ID)]);
        let curr = board(&[(28, PAWN_ID)]);
        assert_eq!(replay_motion_squares(&prev, &curr), vec![(12, 28)]);
    }

    #[test]
    fn a_capture_goes_to_the_victims_square() {
        let prev = board(&[(28, PAWN_ID), (35, -PAWN_ID)]);
        let curr = board(&[(35, PAWN_ID)]);
        assert_eq!(replay_motion_squares(&prev, &curr), vec![(28, 35)]);
    }

    #[test]
    fn castling_moves_the_king_and_the_rook() {
        let prev = board(&[(4, KING_ID), (7, ROOK_ID)]);
        let curr = board(&[(6, KING_ID), (5, ROOK_ID)]);
        assert_eq!(replay_motion_squares(&prev, &curr), vec![(4, 6), (7, 5)]);

        let prev = board(&[(60, -KING_ID), (56, -ROOK_ID)]);
        let curr = board(&[(58, -KING_ID), (59, -ROOK_ID)]);
        assert_eq!(
            replay_motion_squares(&prev, &curr),
            vec![(60, 58), (56, 59)]
        );
    }

    #[test]
    fn promotion_still_pairs_the_pawn_with_its_destination() {
        let prev = board(&[(48, PAWN_ID)]);
        let curr = board(&[(56, QUEEN_ID)]);
        assert_eq!(replay_motion_squares(&prev, &curr), vec![(48, 56)]);
    }

    #[test]
    fn an_unchanged_board_has_no_motion() {
        let same = board(&[(4, KING_ID)]);
        assert!(replay_motion_squares(&same, &same).is_empty());
    }

    #[test]
    fn en_passant_pairs_the_mover_in_both_directions() {
        let prev = board(&[(35, -PAWN_ID), (36, PAWN_ID)]);
        let curr = board(&[(43, PAWN_ID)]);
        assert_eq!(replay_motion_squares(&prev, &curr), vec![(36, 43)]);
        let prev = board(&[(27, -PAWN_ID), (28, PAWN_ID)]);
        let curr = board(&[(20, -PAWN_ID)]);
        assert_eq!(replay_motion_squares(&prev, &curr), vec![(27, 20)]);
    }

    #[test]
    fn promotion_capture_and_large_seek_are_distinguished() {
        let prev = board(&[(48, PAWN_ID), (57, -ROOK_ID)]);
        let curr = board(&[(57, QUEEN_ID)]);
        assert_eq!(replay_motion_squares(&prev, &curr), vec![(48, 57)]);
        let prev = board(&[(12, PAWN_ID), (52, -PAWN_ID)]);
        let curr = board(&[(28, PAWN_ID), (36, -PAWN_ID)]);
        assert!(replay_motion_squares(&prev, &curr).is_empty());
    }
}
