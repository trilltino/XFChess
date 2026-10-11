use super::*;

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

    for sq in 0..64usize {
        let piece_id = curr_board[sq];
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
        let piece_type = engine_id_to_piece_type(piece_id.abs());

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

    // If this was a single forward advance, inject a PieceMoveAnimation tween
    if animate && engine_ply > 0 {
        let mut src_sq: Option<usize> = None;
        let mut dst_sq: Option<usize> = None;
        for sq in 0..64usize {
            let p = prev_board[sq];
            let c = curr_board[sq];
            if p != 0 && c == 0 && src_sq.is_none() {
                src_sq = Some(sq);
            }
            // Destination: piece arrived (was empty) or captured (colour flipped)
            if c != 0
                && p != c
                && dst_sq.is_none()
                && (p == 0 || (p != 0 && p.signum() != c.signum()))
            {
                dst_sq = Some(sq);
            }
        }
        if let (Some(src), Some(dst)) = (src_sq, dst_sq) {
            // World X is mirrored (7 - file), matching spawn_piece_at_replay /
            // execute_move's PieceMoveAnimation targets — see pieces.rs:484.
            let src_world = Vec3::new(7.0 - (src % 8) as f32, PIECE_ON_BOARD_Y, (src / 8) as f32);
            let dst_world = Vec3::new(7.0 - (dst % 8) as f32, PIECE_ON_BOARD_Y, (dst / 8) as f32);
            if let Some(&ent) = entity_at_sq.get(&dst) {
                let duration = 0.3 / slow_factor.max(0.05);
                commands
                    .entity(ent)
                    .insert(PieceMoveAnimation::new(src_world, dst_world, duration));
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

pub(super) fn engine_to_fen(game: &nimzovich_engine::Game) -> String {
    game_to_fen(game)
}

pub(super) fn engine_id_to_piece_type(id: i8) -> PieceType {
    use nimzovich_engine::{BISHOP_ID, KING_ID, KNIGHT_ID, PAWN_ID, QUEEN_ID, ROOK_ID};
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
