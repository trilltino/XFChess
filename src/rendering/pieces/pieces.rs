use crate::game::components::HasMoved;
use crate::game::systems::input::{
    on_piece_click, on_piece_drag, on_piece_drag_end, on_piece_drag_start,
};
use crate::input::pointer::{on_piece_hover, on_piece_unhover};
use bevy::color::Color;

use bevy::camera::visibility::RenderLayers;
use bevy::picking::pointer::PointerInteraction;
use bevy::prelude::*;
use std::f32;

// Re-export piece types from their canonical location in game/components.
// All existing `use crate::rendering::pieces::{Piece, PieceColor, PieceType}` imports
// continue to work without changes.
pub use crate::game::components::piece_types::{Piece, PieceColor, PieceType};

const PIECE_Y_OFFSET: f32 = 0.0;

pub const PIECE_MESH_SCALE: f32 = 1.0;

pub const PIECE_ON_BOARD_Y: f32 = 0.05;

#[derive(Resource, Default)]
pub struct PiecesSpawned {
    pub spawned: bool,
}

pub fn white_piece_material() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgb(0.92, 0.89, 0.82), // warm ivory, not pure white
        perceptual_roughness: 0.25,
        metallic: 0.0,
        reflectance: 0.55,
        ..default()
    }
}

pub fn black_piece_material() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgb(0.10, 0.08, 0.07), // very dark warm brown-black
        perceptual_roughness: 0.20,
        metallic: 0.0,
        reflectance: 0.50,
        ..default()
    }
}

#[derive(Component)]
pub struct Piece3DVisual;

#[derive(Component)]
pub struct Piece2DVisual;

pub fn create_pieces(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    piece_meshes: Res<PieceMeshes>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    _view_mode: Res<crate::game::view_mode::ViewMode>,
    mut pieces_spawned: ResMut<PiecesSpawned>,
    sprite_handles: Option<Res<PieceSpriteHandles>>,
    puzzle_board: Option<Res<crate::puzzle::PuzzleBoard>>,
) {
    // Skip if already spawned
    if pieces_spawned.spawned {
        return;
    }

    // Pieces are rendered in all modes to ensure full game state visibility

    // Check if all piece meshes are loaded
    let meshes_to_check = piece_meshes.all_ids();

    for mesh_id in meshes_to_check.iter() {
        match asset_server.load_state(*mesh_id) {
            bevy::asset::LoadState::Loaded => {}
            _ => {
                info!("[PIECES] Waiting for piece meshes to load...");
                return; // Not all meshes loaded yet, try again next frame
            }
        }
    }

    info!("[PIECES] All piece meshes loaded - spawning pieces");

    // Use the documented constant offset to position pieces on the board surface
    // See PIECE_Y_OFFSET documentation for how to recalculate if models change
    let visual_offset = Vec3::new(0.0, PIECE_Y_OFFSET, 0.0);

    // Puzzle mode: spawn the position described by the FEN instead of the
    // standard starting layout.
    if let Some(pb) = puzzle_board.as_ref() {
        if pb.active && !pb.fen.is_empty() {
            spawn_pieces_from_fen(
                &mut commands,
                &piece_meshes,
                &mut materials,
                &pb.fen,
                visual_offset,
                &sprite_handles,
            );
            pieces_spawned.spawned = true;
            info!("[PIECES] Spawned puzzle position from FEN");
            return;
        }
    }

    // Each piece will get its own unique material to prevent color bleeding
    // during capture animations. This ensures fade effects don't affect other pieces.

    // Data-driven piece placement using standard chess starting positions
    const BACK_ROW: [PieceType; 8] = [
        PieceType::Rook,
        PieceType::Knight,
        PieceType::Bishop,
        PieceType::Queen,
        PieceType::King,
        PieceType::Bishop,
        PieceType::Knight,
        PieceType::Rook,
    ];

    // Spawn white pieces (rank 0 in chess coordinates = rank 1 on board)
    for (file, &piece_type) in BACK_ROW.iter().enumerate() {
        let piece_material = materials.add(white_piece_material());
        spawn_piece_at(
            &mut commands,
            &piece_meshes,
            piece_material,
            PieceColor::White,
            piece_type,
            (file as u8, 0), // (file, rank) -> world (X, Z)
            visual_offset,
            &sprite_handles,
        );
    }

    // Spawn white pawns (rank 1 in chess coordinates = rank 2 on board)
    for file in 0..8 {
        let piece_material = materials.add(white_piece_material());
        spawn_piece_at(
            &mut commands,
            &piece_meshes,
            piece_material,
            PieceColor::White,
            PieceType::Pawn,
            (file as u8, 1), // (file, rank) -> world (X, Z)
            visual_offset,
            &sprite_handles,
        );
    }

    // Spawn black pieces (rank 7 in chess coordinates = rank 8 on board)
    for (file, &piece_type) in BACK_ROW.iter().enumerate() {
        let piece_material = materials.add(black_piece_material());
        spawn_piece_at(
            &mut commands,
            &piece_meshes,
            piece_material,
            PieceColor::Black,
            piece_type,
            (file as u8, 7), // (file, rank) -> world (X, Z)
            visual_offset,
            &sprite_handles,
        );
    }

    // Spawn black pawns (rank 6 in chess coordinates = rank 7 on board)
    for file in 0..8 {
        let piece_material = materials.add(black_piece_material());
        spawn_piece_at(
            &mut commands,
            &piece_meshes,
            piece_material,
            PieceColor::Black,
            PieceType::Pawn,
            (file as u8, 6), // (file, rank) -> world (X, Z)
            visual_offset,
            &sprite_handles,
        );
    }

    pieces_spawned.spawned = true;
    info!("[PIECES] All 32 pieces spawned successfully");
}

#[allow(clippy::too_many_arguments)]
fn spawn_pieces_from_fen(
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
}

#[derive(Resource)]
pub struct PieceMeshes {
    pub white_king: Handle<Mesh>,
    pub white_queen: Handle<Mesh>,
    pub white_rook: Handle<Mesh>,
    pub white_bishop: Handle<Mesh>,
    pub white_knight: Handle<Mesh>,
    pub white_pawn: Handle<Mesh>,
    pub black_king: Handle<Mesh>,
    pub black_queen: Handle<Mesh>,
    pub black_rook: Handle<Mesh>,
    pub black_bishop: Handle<Mesh>,
    pub black_knight: Handle<Mesh>,
    pub black_pawn: Handle<Mesh>,
}

#[derive(Resource)]
pub struct PieceSpriteHandles {
    pub white_king: Handle<Image>,
    pub white_queen: Handle<Image>,
    pub white_rook: Handle<Image>,
    pub white_bishop: Handle<Image>,
    pub white_knight: Handle<Image>,
    pub white_pawn: Handle<Image>,
    pub black_king: Handle<Image>,
    pub black_queen: Handle<Image>,
    pub black_rook: Handle<Image>,
    pub black_bishop: Handle<Image>,
    pub black_knight: Handle<Image>,
    pub black_pawn: Handle<Image>,
}

impl PieceSpriteHandles {
    pub fn get(&self, piece_type: PieceType, color: PieceColor) -> Handle<Image> {
        match (piece_type, color) {
            (PieceType::King, PieceColor::White) => self.white_king.clone(),
            (PieceType::Queen, PieceColor::White) => self.white_queen.clone(),
            (PieceType::Rook, PieceColor::White) => self.white_rook.clone(),
            (PieceType::Bishop, PieceColor::White) => self.white_bishop.clone(),
            (PieceType::Knight, PieceColor::White) => self.white_knight.clone(),
            (PieceType::Pawn, PieceColor::White) => self.white_pawn.clone(),
            (PieceType::King, PieceColor::Black) => self.black_king.clone(),
            (PieceType::Queen, PieceColor::Black) => self.black_queen.clone(),
            (PieceType::Rook, PieceColor::Black) => self.black_rook.clone(),
            (PieceType::Bishop, PieceColor::Black) => self.black_bishop.clone(),
            (PieceType::Knight, PieceColor::Black) => self.black_knight.clone(),
            (PieceType::Pawn, PieceColor::Black) => self.black_pawn.clone(),
        }
    }
}

impl PieceMeshes {
    pub fn get(&self, piece_type: PieceType, color: PieceColor) -> Handle<Mesh> {
        match (piece_type, color) {
            (PieceType::King, PieceColor::White) => self.white_king.clone(),
            (PieceType::Queen, PieceColor::White) => self.white_queen.clone(),
            (PieceType::Rook, PieceColor::White) => self.white_rook.clone(),
            (PieceType::Bishop, PieceColor::White) => self.white_bishop.clone(),
            (PieceType::Knight, PieceColor::White) => self.white_knight.clone(),
            (PieceType::Pawn, PieceColor::White) => self.white_pawn.clone(),
            (PieceType::King, PieceColor::Black) => self.black_king.clone(),
            (PieceType::Queen, PieceColor::Black) => self.black_queen.clone(),
            (PieceType::Rook, PieceColor::Black) => self.black_rook.clone(),
            (PieceType::Bishop, PieceColor::Black) => self.black_bishop.clone(),
            (PieceType::Knight, PieceColor::Black) => self.black_knight.clone(),
            (PieceType::Pawn, PieceColor::Black) => self.black_pawn.clone(),
        }
    }

    pub fn all_ids(&self) -> [bevy::asset::AssetId<Mesh>; 12] {
        [
            self.white_king.id(),
            self.white_queen.id(),
            self.white_rook.id(),
            self.white_bishop.id(),
            self.white_knight.id(),
            self.white_pawn.id(),
            self.black_king.id(),
            self.black_queen.id(),
            self.black_rook.id(),
            self.black_bishop.id(),
            self.black_knight.id(),
            self.black_pawn.id(),
        ]
    }
}

fn piece_set_folder(piece_set: u8) -> &'static str {
    match piece_set {
        1 => "alpha",
        2 => "merida",
        _ => "cburnett",
    }
}

fn load_sprite_handles_for_set(asset_server: &AssetServer, piece_set: u8) -> PieceSpriteHandles {
    let folder = piece_set_folder(piece_set);
    PieceSpriteHandles {
        white_bishop: asset_server.load(format!("pieces/2d/{}/wb.png", folder)),
        white_king: asset_server.load(format!("pieces/2d/{}/wk.png", folder)),
        white_knight: asset_server.load(format!("pieces/2d/{}/wn.png", folder)),
        white_pawn: asset_server.load(format!("pieces/2d/{}/wp.png", folder)),
        white_queen: asset_server.load(format!("pieces/2d/{}/wq.png", folder)),
        white_rook: asset_server.load(format!("pieces/2d/{}/wr.png", folder)),
        black_bishop: asset_server.load(format!("pieces/2d/{}/bb.png", folder)),
        black_king: asset_server.load(format!("pieces/2d/{}/bk.png", folder)),
        black_knight: asset_server.load(format!("pieces/2d/{}/bn.png", folder)),
        black_pawn: asset_server.load(format!("pieces/2d/{}/bp.png", folder)),
        black_queen: asset_server.load(format!("pieces/2d/{}/bq.png", folder)),
        black_rook: asset_server.load(format!("pieces/2d/{}/br.png", folder)),
    }
}

pub fn reload_piece_sprites(
    settings: Res<crate::core::GameSettings>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    if !settings.is_changed() {
        return;
    }
    let sprites = load_sprite_handles_for_set(&asset_server, settings.piece_set);
    commands.insert_resource(sprites);
}

fn load_piece_meshes(mut commands: Commands, asset_server: Res<AssetServer>) {
    info!("[PIECES] Loading piece meshes from wooden_chess_board.glb");

    // Correct mapping from reference ENGINE_TO_MODEL:
    //   Bishop=Mesh0/18, King=Mesh2/20, Knight=Mesh3/21,
    //   Pawn=Mesh5/23, Queen=Mesh13/31, Rook=Mesh14/32
    let meshes = PieceMeshes {
        white_bishop: asset_server.load("models/wooden_chess_board.glb#Mesh18/Primitive0"),
        white_king: asset_server.load("models/wooden_chess_board.glb#Mesh20/Primitive0"),
        white_knight: asset_server.load("models/wooden_chess_board.glb#Mesh21/Primitive0"),
        white_pawn: asset_server.load("models/wooden_chess_board.glb#Mesh23/Primitive0"),
        white_queen: asset_server.load("models/wooden_chess_board.glb#Mesh31/Primitive0"),
        white_rook: asset_server.load("models/wooden_chess_board.glb#Mesh32/Primitive0"),
        black_bishop: asset_server.load("models/wooden_chess_board.glb#Mesh0/Primitive0"),
        black_king: asset_server.load("models/wooden_chess_board.glb#Mesh2/Primitive0"),
        black_knight: asset_server.load("models/wooden_chess_board.glb#Mesh3/Primitive0"),
        black_pawn: asset_server.load("models/wooden_chess_board.glb#Mesh5/Primitive0"),
        black_queen: asset_server.load("models/wooden_chess_board.glb#Mesh13/Primitive0"),
        black_rook: asset_server.load("models/wooden_chess_board.glb#Mesh14/Primitive0"),
    };

    commands.insert_resource(meshes);

    info!("[PIECES] Loading 2D piece sprites from assets/pieces/2d/");
    let sprites = load_sprite_handles_for_set(&asset_server, 0);
    commands.insert_resource(sprites);

    info!("[PIECES] Mesh and Sprite handles created - waiting for assets to load");
}

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

fn append_2d_visual(
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

fn piece_mesh_transform(offset: Vec3) -> Transform {
    let mut t = Transform::from_translation(offset);
    t.scale = Vec3::splat(PIECE_MESH_SCALE);
    t
}

fn piece_rotation(color: PieceColor) -> Quat {
    match color {
        PieceColor::White => Quat::IDENTITY,
        PieceColor::Black => Quat::from_rotation_y(std::f32::consts::PI), // 180 degrees
    }
}

fn knight_rotation(color: PieceColor) -> Quat {
    match color {
        // White: Faces opponent (+Z). Asset faces +X, so rotate +90 degrees.
        PieceColor::White => Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        // Black: Faces opponent (-Z). Asset faces +X, so rotate -90 degrees.
        PieceColor::Black => Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2),
    }
}

fn piece_name(piece_type: PieceType, color: PieceColor, file: u8, rank: u8) -> String {
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
    use crate::core::{DespawnOnExit, GameState};

    let mesh = piece_meshes.get(PieceType::King, piece_color);

    commands
        .spawn((
            Piece::new(piece_color, PieceType::King, file, rank),
            Transform::from_translation(world_pos).with_rotation(piece_rotation(piece_color)),
            GlobalTransform::default(),
            Visibility::default(),
            DespawnOnExit(GameState::InGame),
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
            // Also append 2D visual
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
    use crate::core::GameState;

    let mesh = piece_meshes.get(PieceType::Knight, piece_color);

    commands
        .spawn((
            Piece::new(piece_color, PieceType::Knight, file, rank),
            Transform::from_translation(world_pos).with_rotation(knight_rotation(piece_color)),
            GlobalTransform::default(),
            Visibility::default(),
            DespawnOnExit(GameState::InGame),
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
            // Also append 2D visual
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
    use crate::core::GameState;

    let mesh = piece_meshes.get(PieceType::Queen, piece_color);

    commands
        .spawn((
            Piece::new(piece_color, PieceType::Queen, file, rank),
            Transform::from_translation(world_pos).with_rotation(piece_rotation(piece_color)),
            GlobalTransform::default(),
            Visibility::default(),
            DespawnOnExit(GameState::InGame),
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
            // Also append 2D visual
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
    use crate::core::GameState;

    let mesh = piece_meshes.get(PieceType::Bishop, piece_color);

    commands
        .spawn((
            Piece::new(piece_color, PieceType::Bishop, file, rank),
            Transform::from_translation(world_pos).with_rotation(piece_rotation(piece_color)),
            GlobalTransform::default(),
            Visibility::default(),
            DespawnOnExit(GameState::InGame),
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
            // Also append 2D visual
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
    use crate::core::GameState;

    let mesh = piece_meshes.get(PieceType::Rook, piece_color);

    commands
        .spawn((
            Piece::new(piece_color, PieceType::Rook, file, rank),
            Transform::from_translation(world_pos).with_rotation(piece_rotation(piece_color)),
            GlobalTransform::default(),
            Visibility::default(),
            DespawnOnExit(GameState::InGame),
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
            // Also append 2D visual
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
    use crate::core::GameState;

    let mesh = piece_meshes.get(PieceType::Pawn, piece_color);

    commands
        .spawn((
            Piece::new(piece_color, PieceType::Pawn, file, rank),
            Transform::from_translation(world_pos).with_rotation(piece_rotation(piece_color)),
            GlobalTransform::default(),
            Visibility::default(),
            DespawnOnExit(GameState::InGame),
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
            // Also append 2D visual
            append_2d_visual(parent, piece_color, PieceType::Pawn, sprite_handles);
        });
}

#[derive(Resource)]
pub struct PiecePickingAssets {
    pub mesh_2d: Handle<Mesh>,
    pub matl: Handle<StandardMaterial>,
}

#[derive(Component)]
pub struct PiecePickingProxy2D;

fn init_piece_picking_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    // Flat slab covers the full cell, sits at Y=0.06 (just above the board surface at Y=0.05).
    // In top-down view the slab's top face matches the 2D sprite footprint exactly.
    let mesh_2d = meshes.add(Cuboid::new(0.98, 0.02, 0.98));
    let matl = mats.add(StandardMaterial {
        base_color: Color::srgba(0.0, 0.0, 0.0, 0.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.insert_resource(PiecePickingAssets { mesh_2d, matl });
}

fn on_piece_added(
    trigger: On<bevy::ecs::lifecycle::Add, Piece>,
    mut commands: Commands,
    assets: Option<Res<PiecePickingAssets>>,
) {
    let Some(assets) = assets else { return };
    commands
        .entity(trigger.event_target())
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(assets.mesh_2d.clone()),
                MeshMaterial3d(assets.matl.clone()),
                Transform::from_xyz(0.0, 0.06, 0.0),
                PiecePickingProxy2D,
                bevy::picking::Pickable::IGNORE, // activated by view_mode_rendering_toggle_system
                RenderLayers::layer(crate::game::systems::camera::BOARD_LAYER),
                Name::new("Piece Picking Proxy 2D"),
            ));
        });
}

pub struct PiecePlugin;
impl Plugin for PiecePlugin {
    fn build(&self, app: &mut App) {
        use crate::core::GameState;
        app.init_resource::<PiecesSpawned>();
        app.add_systems(Startup, (load_piece_meshes, init_piece_picking_assets));
        app.add_systems(Update, create_pieces.run_if(in_state(GameState::InGame)));
        app.add_systems(OnExit(GameState::InGame), reset_pieces_spawned);
        // Apply the current view mode's visibility on game entry (idempotent),
        // then keep it applied whenever the mode changes or pieces (re)spawn.
        // `ViewMode` is the single source of truth, so this can never desync.
        app.add_systems(
            OnEnter(GameState::InGame),
            view_mode_rendering_toggle_system,
        );
        app.add_systems(
            Update,
            view_mode_rendering_toggle_system.run_if(
                in_state(GameState::InGame).and(
                    resource_changed::<crate::game::view_mode::ViewMode>
                        .or(resource_changed::<PiecesSpawned>),
                ),
            ),
        );
        app.add_observer(on_piece_added);
    }
}

pub fn view_mode_rendering_toggle_system(
    view_mode: Res<crate::game::view_mode::ViewMode>,
    mut piece_3d_query: Query<
        (&mut Visibility, &mut bevy::picking::Pickable),
        (With<Piece3DVisual>, Without<Piece2DVisual>),
    >,
    mut piece_2d_query: Query<&mut Visibility, (With<Piece2DVisual>, Without<Piece3DVisual>)>,
    mut proxy_2d_query: Query<
        &mut bevy::picking::Pickable,
        (With<PiecePickingProxy2D>, Without<Piece3DVisual>),
    >,
) {
    let mode = *view_mode;
    let (show_3d, show_2d) = match mode {
        crate::game::view_mode::ViewMode::Standard3D => (true, false),
        crate::game::view_mode::ViewMode::Standard2D => (false, true),
        #[cfg(feature = "templeos")]
        crate::game::view_mode::ViewMode::TempleOS => (true, false),
    };

    for (mut vis, mut pick) in piece_3d_query.iter_mut() {
        *vis = if show_3d {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        // In 2D mode disable 3D mesh picking so only the flat 2D proxy receives clicks.
        *pick = if show_3d {
            bevy::picking::Pickable::default()
        } else {
            bevy::picking::Pickable::IGNORE
        };
    }
    for mut vis in piece_2d_query.iter_mut() {
        *vis = if show_2d {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    for mut pick in proxy_2d_query.iter_mut() {
        *pick = if show_2d {
            bevy::picking::Pickable::default()
        } else {
            bevy::picking::Pickable::IGNORE
        };
    }
}

fn reset_pieces_spawned(mut pieces_spawned: ResMut<PiecesSpawned>) {
    pieces_spawned.spawned = false;
    info!("[PIECES] Reset spawn flag for next game");
}
