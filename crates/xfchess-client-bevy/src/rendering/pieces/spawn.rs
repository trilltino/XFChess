use super::*;

macro_rules! spawn_piece_visual {
    ($parent:expr, $mesh:expr, $material:expr, $offset:expr) => {
        $parent.spawn((
            Mesh3d($mesh),
            MeshMaterial3d($material),
            piece_mesh_transform($offset),
            Piece3DVisual,
            bevy::picking::Pickable::default(), // actual mesh IS the 3D hit target
            RenderLayers::layer(crate::game::systems::camera::BOARD_LAYER),
        ));
    };
}

/// Spawn pieces from a FEN board. FEN row i maps to board rank 7-i.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_pieces_from_fen(
    commands: &mut Commands,
    meshes: &PieceMeshes,
    materials: &mut Assets<StandardMaterial>,
    fen: &str,
    visual_offset: Vec3,
    sprite_handles: &Option<Res<PieceSpriteHandles>>,
) {
    let board = fen.split_whitespace().next().unwrap_or("");
    for (row_idx, row) in board.split('/').enumerate() {
        if row_idx >= 8 {
            break;
        }
        let rank = 7u8.saturating_sub(row_idx as u8);
        let mut file: u8 = 0;
        for ch in row.chars() {
            if let Some(d) = ch.to_digit(10) {
                file = file.saturating_add(d as u8);
                continue;
            }
            let color = if ch.is_ascii_uppercase() {
                PieceColor::White
            } else {
                PieceColor::Black
            };
            let piece_type = match ch.to_ascii_lowercase() {
                'k' => PieceType::King,
                'q' => PieceType::Queen,
                'r' => PieceType::Rook,
                'b' => PieceType::Bishop,
                'n' => PieceType::Knight,
                'p' => PieceType::Pawn,
                _ => {
                    file = file.saturating_add(1);
                    continue;
                }
            };
            if file < 8 {
                let material = if color == PieceColor::White {
                    materials.add(white_piece_material())
                } else {
                    materials.add(black_piece_material())
                };
                spawn_piece_at(
                    commands,
                    meshes,
                    material,
                    color,
                    piece_type,
                    (file, rank),
                    visual_offset,
                    sprite_handles,
                );
            }
            file = file.saturating_add(1);
        }
    }
    let fen = fen.to_owned();
    commands.queue(move |world: &mut World| {
        crate::game::systems::board_snapshot::stamp_fen_history(world, &fen)
    });
}

/// Spawn a piece at zero-based (file, rank) coordinates. Mesh offsets compensate
/// for different GLB origins and center each piece at local X/Z = 0.5 after rotation.
pub fn spawn_piece_at(
    commands: &mut Commands,
    meshes: &PieceMeshes,
    material: Handle<StandardMaterial>,
    color: PieceColor,
    piece_type: PieceType,
    position: (u8, u8),
    _visual_offset: Vec3, // Kept for API compatibility - reference assets are already centered
    sprite_handles: &Option<Res<PieceSpriteHandles>>,
) {
    let (file, rank) = position;
    // World position: X = 7-file (mirrored so a-file is left from White camera), Y = board surface, Z = rank
    let world_pos = Vec3::new(7.0 - file as f32, PIECE_ON_BOARD_Y, rank as f32);

    // Reference assets are already centered - no offsets needed
    let offset = Vec3::ZERO;

    // Handle is Clone (not Copy), need .clone() from shared ref
    match piece_type {
        PieceType::King => spawn_king(
            commands,
            material,
            color,
            world_pos,
            meshes,
            offset,
            file,
            rank,
            sprite_handles,
        ),
        PieceType::Queen => spawn_queen(
            commands,
            material,
            color,
            world_pos,
            meshes,
            offset,
            file,
            rank,
            sprite_handles,
        ),
        PieceType::Rook => spawn_rook(
            commands,
            material,
            color,
            world_pos,
            meshes,
            offset,
            file,
            rank,
            sprite_handles,
        ),
        PieceType::Bishop => spawn_bishop(
            commands,
            material,
            color,
            world_pos,
            meshes,
            offset,
            file,
            rank,
            sprite_handles,
        ),
        PieceType::Knight => spawn_knight(
            commands,
            material,
            color,
            world_pos,
            meshes,
            offset,
            file,
            rank,
            sprite_handles,
        ),
        PieceType::Pawn => spawn_pawn(
            commands,
            material,
            color,
            world_pos,
            meshes,
            offset,
            file,
            rank,
            sprite_handles,
        ),
    }

    // After spawning the piece wrapper, we also need to attach the 2D visual
    // This is handled in each spawn_X function via with_children
}

pub(super) fn append_2d_visual(
    parent: &mut ChildSpawnerCommands,
    color: PieceColor,
    piece_type: PieceType,
    sprite_handles: &Option<Res<PieceSpriteHandles>>,
) {
    if let Some(handles) = sprite_handles {
        let sprite = handles.get(piece_type, color);
        parent.spawn((
            Sprite::from_image(sprite),
            // Position 2D sprite slightly above board surface (Y=0.1)
            // Rotate to face camera (X=-90deg)
            Transform::from_xyz(0.0, 0.1, 0.0)
                .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
                .with_scale(Vec3::splat(0.002)), // Scale appropriate for 1.0x1.0 square
            Piece2DVisual,
            Visibility::Hidden, // Hidden by default (start in 3D)
        ));
    }
}

pub(super) fn piece_mesh_transform(offset: Vec3) -> Transform {
    let mut t = Transform::from_translation(offset);
    t.scale = Vec3::splat(PIECE_MESH_SCALE);
    t
}

/// Get rotation for piece based on color - black pieces face opposite direction
pub(super) fn piece_rotation(color: PieceColor) -> Quat {
    match color {
        PieceColor::White => Quat::IDENTITY,
        PieceColor::Black => Quat::from_rotation_y(std::f32::consts::PI), // 180 degrees
    }
}

/// Rotate knights 90 degrees because their GLB faces +X instead of +Z.
pub(super) fn knight_rotation(color: PieceColor) -> Quat {
    match color {
        // White: Faces opponent (+Z). Asset faces +X, so rotate +90 degrees.
        PieceColor::White => Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        // Black: Faces opponent (-Z). Asset faces +X, so rotate -90 degrees.
        PieceColor::Black => Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2),
    }
}

/// Inspector piece name from zero-based file (a-h) and rank (1-8).
pub(super) fn piece_name(piece_type: PieceType, color: PieceColor, file: u8, rank: u8) -> String {
    let color_str = match color {
        PieceColor::White => "White",
        PieceColor::Black => "Black",
    };
    let piece_str = match piece_type {
        PieceType::King => "King",
        PieceType::Queen => "Queen",
        PieceType::Rook => "Rook",
        PieceType::Bishop => "Bishop",
        PieceType::Knight => "Knight",
        PieceType::Pawn => "Pawn",
    };
    let file_char = (b'a' + file) as char;
    let rank_num = rank + 1;
    format!("{} {} {}{}", color_str, piece_str, file_char, rank_num)
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_king(
    commands: &mut Commands,
    material: Handle<StandardMaterial>,
    piece_color: PieceColor,
    world_pos: Vec3,
    piece_meshes: &PieceMeshes,
    _visual_offset: Vec3,
    file: u8,
    rank: u8,
    sprite_handles: &Option<Res<PieceSpriteHandles>>,
) {
    let mesh = piece_meshes.get(PieceType::King, piece_color);

    commands
        .spawn((
            Piece::new(piece_color, PieceType::King, file, rank),
            Transform::from_translation(world_pos).with_rotation(piece_rotation(piece_color)),
            GlobalTransform::default(),
            Visibility::default(),
            crate::core::game_scene(),
            PointerInteraction::default(),
            bevy::picking::Pickable::default(),
            RenderLayers::layer(crate::game::systems::camera::BOARD_LAYER),
            Name::new(piece_name(PieceType::King, piece_color, file, rank)),
            HasMoved::default(),
        ))
        .observe(on_piece_click)
        .observe(on_piece_drag_start)
        .observe(on_piece_drag)
        .observe(on_piece_drag_end)
        .observe(on_piece_hover)
        .observe(on_piece_unhover)
        .with_children(|parent| {
            spawn_piece_visual!(parent, mesh, material, Vec3::ZERO);
            append_2d_visual(parent, piece_color, PieceType::King, sprite_handles);
        });
}

pub fn spawn_knight(
    commands: &mut Commands,
    material: Handle<StandardMaterial>,
    piece_color: PieceColor,
    world_pos: Vec3,
    piece_meshes: &PieceMeshes,
    _visual_offset: Vec3,
    file: u8,
    rank: u8,
    sprite_handles: &Option<Res<PieceSpriteHandles>>,
) {
    let mesh = piece_meshes.get(PieceType::Knight, piece_color);

    commands
        .spawn((
            Piece::new(piece_color, PieceType::Knight, file, rank),
            Transform::from_translation(world_pos).with_rotation(knight_rotation(piece_color)),
            GlobalTransform::default(),
            Visibility::default(),
            crate::core::game_scene(),
            PointerInteraction::default(),
            bevy::picking::Pickable::default(),
            RenderLayers::layer(crate::game::systems::camera::BOARD_LAYER),
            Name::new(piece_name(PieceType::Knight, piece_color, file, rank)),
            HasMoved::default(),
        ))
        .observe(on_piece_click)
        .observe(on_piece_drag_start)
        .observe(on_piece_drag)
        .observe(on_piece_drag_end)
        .observe(on_piece_hover)
        .observe(on_piece_unhover)
        .with_children(|parent| {
            spawn_piece_visual!(parent, mesh, material, Vec3::ZERO);
            append_2d_visual(parent, piece_color, PieceType::Knight, sprite_handles);
        });
}

pub fn spawn_queen(
    commands: &mut Commands,
    material: Handle<StandardMaterial>,
    piece_color: PieceColor,
    world_pos: Vec3,
    piece_meshes: &PieceMeshes,
    _visual_offset: Vec3,
    file: u8,
    rank: u8,
    sprite_handles: &Option<Res<PieceSpriteHandles>>,
) {
    let mesh = piece_meshes.get(PieceType::Queen, piece_color);

    commands
        .spawn((
            Piece::new(piece_color, PieceType::Queen, file, rank),
            Transform::from_translation(world_pos).with_rotation(piece_rotation(piece_color)),
            GlobalTransform::default(),
            Visibility::default(),
            crate::core::game_scene(),
            PointerInteraction::default(),
            bevy::picking::Pickable::default(),
            RenderLayers::layer(crate::game::systems::camera::BOARD_LAYER),
            Name::new(piece_name(PieceType::Queen, piece_color, file, rank)),
            HasMoved::default(),
        ))
        .observe(on_piece_click)
        .observe(on_piece_drag_start)
        .observe(on_piece_drag)
        .observe(on_piece_drag_end)
        .observe(on_piece_hover)
        .observe(on_piece_unhover)
        .with_children(|parent| {
            spawn_piece_visual!(parent, mesh, material, Vec3::ZERO);
            append_2d_visual(parent, piece_color, PieceType::Queen, sprite_handles);
        });
}

pub fn spawn_bishop(
    commands: &mut Commands,
    material: Handle<StandardMaterial>,
    piece_color: PieceColor,
    world_pos: Vec3,
    piece_meshes: &PieceMeshes,
    _visual_offset: Vec3,
    file: u8,
    rank: u8,
    sprite_handles: &Option<Res<PieceSpriteHandles>>,
) {
    let mesh = piece_meshes.get(PieceType::Bishop, piece_color);

    commands
        .spawn((
            Piece::new(piece_color, PieceType::Bishop, file, rank),
            Transform::from_translation(world_pos).with_rotation(piece_rotation(piece_color)),
            GlobalTransform::default(),
            Visibility::default(),
            crate::core::game_scene(),
            PointerInteraction::default(),
            bevy::picking::Pickable::default(),
            RenderLayers::layer(crate::game::systems::camera::BOARD_LAYER),
            Name::new(piece_name(PieceType::Bishop, piece_color, file, rank)),
            HasMoved::default(),
        ))
        .observe(on_piece_click)
        .observe(on_piece_drag_start)
        .observe(on_piece_drag)
        .observe(on_piece_drag_end)
        .observe(on_piece_hover)
        .observe(on_piece_unhover)
        .with_children(|parent| {
            spawn_piece_visual!(parent, mesh, material, Vec3::ZERO);
            append_2d_visual(parent, piece_color, PieceType::Bishop, sprite_handles);
        });
}

pub fn spawn_rook(
    commands: &mut Commands,
    material: Handle<StandardMaterial>,
    piece_color: PieceColor,
    world_pos: Vec3,
    piece_meshes: &PieceMeshes,
    _visual_offset: Vec3,
    file: u8,
    rank: u8,
    sprite_handles: &Option<Res<PieceSpriteHandles>>,
) {
    let mesh = piece_meshes.get(PieceType::Rook, piece_color);

    commands
        .spawn((
            Piece::new(piece_color, PieceType::Rook, file, rank),
            Transform::from_translation(world_pos).with_rotation(piece_rotation(piece_color)),
            GlobalTransform::default(),
            Visibility::default(),
            crate::core::game_scene(),
            PointerInteraction::default(),
            bevy::picking::Pickable::default(),
            RenderLayers::layer(crate::game::systems::camera::BOARD_LAYER),
            Name::new(piece_name(PieceType::Rook, piece_color, file, rank)),
            HasMoved::default(),
        ))
        .observe(on_piece_click)
        .observe(on_piece_drag_start)
        .observe(on_piece_drag)
        .observe(on_piece_drag_end)
        .observe(on_piece_hover)
        .observe(on_piece_unhover)
        .with_children(|parent| {
            spawn_piece_visual!(parent, mesh, material, Vec3::ZERO);
            append_2d_visual(parent, piece_color, PieceType::Rook, sprite_handles);
        });
}

pub fn spawn_pawn(
    commands: &mut Commands,
    material: Handle<StandardMaterial>,
    piece_color: PieceColor,
    world_pos: Vec3,
    piece_meshes: &PieceMeshes,
    _visual_offset: Vec3,
    file: u8,
    rank: u8,
    sprite_handles: &Option<Res<PieceSpriteHandles>>,
) {
    let mesh = piece_meshes.get(PieceType::Pawn, piece_color);

    commands
        .spawn((
            Piece::new(piece_color, PieceType::Pawn, file, rank),
            Transform::from_translation(world_pos).with_rotation(piece_rotation(piece_color)),
            GlobalTransform::default(),
            Visibility::default(),
            crate::core::game_scene(),
            PointerInteraction::default(),
            bevy::picking::Pickable::default(),
            RenderLayers::layer(crate::game::systems::camera::BOARD_LAYER),
            Name::new(piece_name(PieceType::Pawn, piece_color, file, rank)),
            HasMoved::default(),
        ))
        .observe(on_piece_click)
        .observe(on_piece_drag_start)
        .observe(on_piece_drag)
        .observe(on_piece_drag_end)
        .observe(on_piece_hover)
        .observe(on_piece_unhover)
        .with_children(|parent| {
            spawn_piece_visual!(parent, mesh, material, Vec3::ZERO);
            append_2d_visual(parent, piece_color, PieceType::Pawn, sprite_handles);
        });
}
