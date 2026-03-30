//! Chess piece 3D rendering — Data-driven GLTF model spawning.
//!
//! The authoritative type definitions for [`Piece`], [`PieceColor`], and
//! [`PieceType`] live in [`crate::game::components::piece_types`].
//! This module re-exports them for backward compatibility.

use crate::game::components::HasMoved;
use crate::game::systems::input::{
    on_piece_click, on_piece_drag, on_piece_drag_end, on_piece_drag_start,
};
use crate::input::pointer::{on_piece_hover, on_piece_unhover};
use bevy::color::Color;

use bevy::picking::pointer::PointerInteraction;
use bevy::prelude::*;
use std::f32;

// Re-export piece types from their canonical location in game/components.
// All existing `use crate::rendering::pieces::{Piece, PieceColor, PieceType}` imports
// continue to work without changes.
pub use crate::game::components::piece_types::{Piece, PieceColor, PieceType};

/// Visual Y offset for piece meshes to align with the board surface.
///
/// The chess kit GLB models have their origin at the BASE of the piece (not geometric center).
/// This means offset Y=0 places the piece base at the parent's Y position.
///
/// The parent entity is positioned at PIECE_ON_BOARD_Y (board surface).
/// With offset Y=0, the piece base sits exactly on the board surface.
const PIECE_Y_OFFSET: f32 = 0.0;

/// Scale factor for piece meshes — fits the chess kit models to board squares
/// Reference uses 1.0 scale for wooden_chess_board.glb
pub const PIECE_MESH_SCALE: f32 = 1.0;

/// Y position for piece parent entities on the board.
///
/// The board squares are `Cuboid::new(1.0, 0.1, 1.0)` centered at y=0,
/// so the top face is at y=0.05. Placing pieces at y=0.05 puts their base
/// flush with the board surface, preventing clipping into the board geometry.
pub const PIECE_ON_BOARD_Y: f32 = 0.05;

/// Resource to track if pieces have been spawned for current game
#[derive(Resource, Default)]
pub struct PiecesSpawned {
    pub spawned: bool,
}

/// Data-driven piece setup - idiomatic Bevy approach
///
/// Uses const arrays to define starting positions, then iterates to spawn pieces.
/// This pattern is cleaner, more maintainable, and easier to test than manual spawning.
///
/// Reference: `reference/bevy/examples/ecs/` for data-driven entity spawning patterns
pub fn create_pieces(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    piece_meshes: Res<PieceMeshes>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    view_mode: Res<crate::game::view_mode::ViewMode>,
    mut pieces_spawned: ResMut<PiecesSpawned>,
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
        // Create unique material for each piece to prevent color bleeding during capture
        let piece_material = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            ..default()
        });
        spawn_piece_at(
            &mut commands,
            &piece_meshes,
            piece_material,
            PieceColor::White,
            piece_type,
            (file as u8, 0), // (file, rank) -> world (X, Z)
            visual_offset,
        );
    }

    // Spawn white pawns (rank 1 in chess coordinates = rank 2 on board)
    for file in 0..8 {
        // Create unique material for each piece to prevent color bleeding during capture
        let piece_material = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            ..default()
        });
        spawn_piece_at(
            &mut commands,
            &piece_meshes,
            piece_material,
            PieceColor::White,
            PieceType::Pawn,
            (file, 1), // (file, rank) -> world (X, Z)
            visual_offset,
        );
    }

    // Spawn black pieces (rank 7 in chess coordinates = rank 8 on board)
    for (file, &piece_type) in BACK_ROW.iter().enumerate() {
        // Create unique material for each piece to prevent color bleeding during capture
        let piece_material = materials.add(StandardMaterial {
            base_color: Color::BLACK,
            ..default()
        });
        spawn_piece_at(
            &mut commands,
            &piece_meshes,
            piece_material,
            PieceColor::Black,
            piece_type,
            (file as u8, 7), // (file, rank) -> world (X, Z)
            visual_offset,
        );
    }

    // Spawn black pawns (rank 6 in chess coordinates = rank 7 on board)
    for file in 0..8 {
        // Create unique material for each piece to prevent color bleeding during capture
        let piece_material = materials.add(StandardMaterial {
            base_color: Color::BLACK,
            ..default()
        });
        spawn_piece_at(
            &mut commands,
            &piece_meshes,
            piece_material,
            PieceColor::Black,
            PieceType::Pawn,
            (file, 6), // (file, rank) -> world (X, Z)
            visual_offset,
        );
    }

    pieces_spawned.spawned = true;
    info!("[PIECES] All 32 pieces spawned successfully");
}

/// Container for piece mesh handles - using wooden_chess_board.glb like the reference
///
/// Mesh indices derived from reference ENGINE_TO_MODEL mapping:
///   Bishop=Mesh0/18, King=Mesh2/20, Knight=Mesh3/21,
///   Pawn=Mesh5/23, Queen=Mesh13/31, Rook=Mesh14/32
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
            self.white_king.id(), self.white_queen.id(), self.white_rook.id(),
            self.white_bishop.id(), self.white_knight.id(), self.white_pawn.id(),
            self.black_king.id(), self.black_queen.id(), self.black_rook.id(),
            self.black_bishop.id(), self.black_knight.id(), self.black_pawn.id(),
        ]
    }
}

fn load_piece_meshes(mut commands: Commands, asset_server: Res<AssetServer>) {
    info!("[PIECES] Loading piece meshes from wooden_chess_board.glb");
    
    // Correct mapping from reference ENGINE_TO_MODEL:
    //   Bishop=Mesh0/18, King=Mesh2/20, Knight=Mesh3/21,
    //   Pawn=Mesh5/23, Queen=Mesh13/31, Rook=Mesh14/32
    let meshes = PieceMeshes {
        white_bishop: asset_server.load("models/wooden_chess_board.glb#Mesh18/Primitive0"),
        white_king:   asset_server.load("models/wooden_chess_board.glb#Mesh20/Primitive0"),
        white_knight: asset_server.load("models/wooden_chess_board.glb#Mesh21/Primitive0"),
        white_pawn:   asset_server.load("models/wooden_chess_board.glb#Mesh23/Primitive0"),
        white_queen:  asset_server.load("models/wooden_chess_board.glb#Mesh31/Primitive0"),
        white_rook:   asset_server.load("models/wooden_chess_board.glb#Mesh32/Primitive0"),
        black_bishop: asset_server.load("models/wooden_chess_board.glb#Mesh0/Primitive0"),
        black_king:   asset_server.load("models/wooden_chess_board.glb#Mesh2/Primitive0"),
        black_knight: asset_server.load("models/wooden_chess_board.glb#Mesh3/Primitive0"),
        black_pawn:   asset_server.load("models/wooden_chess_board.glb#Mesh5/Primitive0"),
        black_queen:  asset_server.load("models/wooden_chess_board.glb#Mesh13/Primitive0"),
        black_rook:   asset_server.load("models/wooden_chess_board.glb#Mesh14/Primitive0"),
    };
    
    commands.insert_resource(meshes);
    info!("[PIECES] Mesh handles created - waiting for assets to load");
}

/// Per-piece-type offsets to center meshes on squares.
///
/// These offsets compensate for the GLB mesh origins being at different positions
/// within the model file. Each piece type in the chess_kit has its mesh center
/// at a different location, requiring specific offsets to center the piece on its square.
///
/// # Coordinate System for Offsets
///
/// Offsets are in the piece's local coordinate space (after Y-rotation is applied):
/// - X: Left/right adjustment to center on square (0.5 = center of 1.0 wide square)
/// - Y: Vertical offset (0.0 = piece base at parent Y position)
/// - Z: Forward/back adjustment to center on square (0.5 = center of 1.0 deep square)
///
/// # Why Different Z Offsets?
///
/// The chess_kit GLB file has each piece type at a different Z position:
/// - King at Z ≈ -1.9, Queen at Z ≈ -0.95, Bishop at Z ≈ 0.0,
/// - Knight at Z ≈ 0.9, Rook at Z ≈ 1.8, Pawn at Z ≈ 2.6
///
/// These offsets bring each piece to the center of its square (local X=0.5, Z=0.5).
/// Y=0.0 keeps the piece at the parent's Y position (PIECE_ON_BOARD_Y = 0.05).
/// The parent entity is positioned at the square corner, so we add 0.5 to center it.
pub const KING_OFFSET: Vec3 = Vec3::new(0.0, 0.0, 1.83);
pub const QUEEN_OFFSET: Vec3 = Vec3::new(0.0, 0.0, 0.92);
pub const BISHOP_OFFSET: Vec3 = Vec3::new(0.0, 0.0, 0.0);
pub const KNIGHT_OFFSET: Vec3 = Vec3::new(0.0, 0.0, -0.92);
pub const ROOK_OFFSET: Vec3 = Vec3::new(0.0, 0.0, -1.82);
pub const PAWN_OFFSET: Vec3 = Vec3::new(0.0, 0.0, -2.63);

/// Unified piece spawning function - dispatches to specific spawner based on type
///
/// # Arguments
/// * `position` - Tuple of (file, rank) where:
///   - file: 0-7 (corresponds to files a-h)
///   - rank: 0-7 (corresponds to ranks 1-8)
pub fn spawn_piece_at(
    commands: &mut Commands,
    meshes: &PieceMeshes,
    material: Handle<StandardMaterial>,
    color: PieceColor,
    piece_type: PieceType,
    position: (u8, u8),
    _visual_offset: Vec3, // Kept for API compatibility - reference assets are already centered
) {
    let (file, rank) = position;
    // World position: X = file, Y = board surface, Z = rank
    // GLB meshes from wooden_chess_board.glb are designed for integer-coordinate placement
    let world_pos = Vec3::new(file as f32, PIECE_ON_BOARD_Y, rank as f32);

    // Reference assets are already centered - no offsets needed
    let offset = Vec3::ZERO;

    // Handle is Clone (not Copy), need .clone() from shared ref
    match piece_type {
        PieceType::King => spawn_king(
            commands, material, color, world_pos, meshes, offset, file, rank,
        ),
        PieceType::Queen => spawn_queen(
            commands, material, color, world_pos, meshes, offset, file, rank,
        ),
        PieceType::Rook => spawn_rook(
            commands, material, color, world_pos, meshes, offset, file, rank,
        ),
        PieceType::Bishop => spawn_bishop(
            commands, material, color, world_pos, meshes, offset, file, rank,
        ),
        PieceType::Knight => spawn_knight(
            commands, material, color, world_pos, meshes, offset, file, rank,
        ),
        PieceType::Pawn => spawn_pawn(
            commands, material, color, world_pos, meshes, offset, file, rank,
        ),
    }
}

fn piece_mesh_transform(offset: Vec3) -> Transform {
    let mut t = Transform::from_translation(offset);
    t.scale = Vec3::splat(PIECE_MESH_SCALE);
    t
}

/// Get rotation for piece based on color - black pieces face opposite direction
fn piece_rotation(color: PieceColor) -> Quat {
    match color {
        PieceColor::White => Quat::IDENTITY,
        PieceColor::Black => Quat::from_rotation_y(std::f32::consts::PI), // 180 degrees
    }
}

/// Get rotation for knights - they need special handling because the GLB model
/// is oriented facing +X (along the board) instead of +Z (across the board).
/// This function adds a 90° rotation to make knights face the opponent.
fn knight_rotation(color: PieceColor) -> Quat {
    match color {
        // White: Faces opponent (+Z). Asset faces +X, so rotate +90 degrees.
        PieceColor::White => Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        // Black: Faces opponent (-Z). Asset faces +X, so rotate -90 degrees.
        PieceColor::Black => Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2),
    }
}

/// Helper function to generate piece name for inspector
///
/// # Arguments
/// * `file` - File index 0-7 (a-h)
/// * `rank` - Rank index 0-7 (1-8)
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
            // Pickable required for child meshes to generate pointer events
            // Events bubble up to parent entity where observers are registered
            bevy::picking::Pickable::default(),
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
        });
}

/// Calculate capture zone position for a captured piece
///
/// Arranges captured pieces on the sides of the board:
/// - White captured pieces (black pieces taken): Left side (x = -2.0 to -1.0)
/// - Black captured pieces (white pieces taken): Right side (x = 8.0 to 9.0)
///
/// Pieces are arranged by type in rows:
/// - Row 0: Pawns
/// - Row 1: Knights
/// - Row 2: Bishops
/// - Row 3: Rooks
/// - Row 4: Queens
///
/// Within each row, pieces are placed horizontally based on count.
pub fn calculate_capture_position(
    captured_piece_color: PieceColor,
    piece_type: PieceType,
    count_of_same_type: usize,
) -> Vec3 {
    // Determine which side (left for white captures, right for black captures)
    let x = match captured_piece_color {
        PieceColor::White => -1.5, // Left side (white captured pieces = black pieces taken)
        PieceColor::Black => 8.5,  // Right side (black captured pieces = white pieces taken)
    };

    // Determine row based on piece type
    let z = match piece_type {
        PieceType::Pawn => 0.0,
        PieceType::Knight => 1.0,
        PieceType::Bishop => 2.0,
        PieceType::Rook => 3.0,
        PieceType::Queen => 4.0,
        PieceType::King => 5.0, // Shouldn't happen, but just in case
    };

    // Position within row (spread horizontally)
    let x_offset = (count_of_same_type as f32) * 0.3;
    let final_x = if captured_piece_color == PieceColor::White {
        x - x_offset
    } else {
        x + x_offset
    };

    Vec3::new(final_x, 0.0, z)
}

pub struct PiecePlugin;
impl Plugin for PiecePlugin {
    fn build(&self, app: &mut App) {
        use crate::core::GameState;
        app.init_resource::<PiecesSpawned>();
        app.add_systems(Startup, load_piece_meshes);
        // Run create_pieces continuously during InGame so it can wait for assets
        app.add_systems(Update, create_pieces.run_if(in_state(GameState::InGame)));
        // Reset spawn flag when leaving InGame so pieces can be respawned next game
        app.add_systems(OnExit(GameState::InGame), reset_pieces_spawned);
    }
}

fn reset_pieces_spawned(mut pieces_spawned: ResMut<PiecesSpawned>) {
    pieces_spawned.spawned = false;
    info!("[PIECES] Reset spawn flag for next game");
}
