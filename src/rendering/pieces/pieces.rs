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
/// Note: This must be 0.2 to match the GLTF mesh offsets (designed for bevy_chess reference)
const PIECE_MESH_SCALE: f32 = 0.2;

// ============================================================================
// GLB MESH OFFSETS (from main_menu_showcase.rs - VERIFIED WORKING)
// ============================================================================
// These are the EXACT same offsets used in main_menu_showcase.rs.
// The showcase multiplies these by OFFSET_RATIO (0.6) because it uses scale 0.12.
// The game uses scale 0.2 directly, so we apply these offsets WITHOUT scaling.

const PAWN_MESH_OFFSET: Vec3 = Vec3::new(-0.2, 0.0, 2.6);
const KNIGHT_1_MESH_OFFSET: Vec3 = Vec3::new(-0.2, 0.0, 0.9);
const KNIGHT_2_MESH_OFFSET: Vec3 = Vec3::new(-0.2, 0.0, 0.9);
const BISHOP_MESH_OFFSET: Vec3 = Vec3::new(-0.1, 0.0, 0.0);
const ROOK_MESH_OFFSET: Vec3 = Vec3::new(-0.1, 0.0, 1.8);
const QUEEN_MESH_OFFSET: Vec3 = Vec3::new(-0.2, 0.0, -0.95);
const KING_BASE_MESH_OFFSET: Vec3 = Vec3::new(-0.2, 0.0, -1.9);
const KING_CROSS_MESH_OFFSET: Vec3 = Vec3::new(-0.2, 0.0, -1.9);

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

    // Skip piece creation in TempleOS mode
    if *view_mode == crate::game::view_mode::ViewMode::TempleOS {
        info!("[PIECES] Skipping piece creation - TempleOS view mode active");
        return;
    }

    // Check if all piece meshes are loaded
    let meshes_to_check = [
        piece_meshes.king.id(),
        piece_meshes.king_cross.id(),
        piece_meshes.queen.id(),
        piece_meshes.rook.id(),
        piece_meshes.bishop.id(),
        piece_meshes.knight_1.id(),
        piece_meshes.knight_2.id(),
        piece_meshes.pawn.id(),
    ];

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

/// Container for piece mesh handles
#[derive(Resource)]
pub struct PieceMeshes {
    pub king: Handle<Mesh>,
    pub king_cross: Handle<Mesh>,
    pub pawn: Handle<Mesh>,
    pub knight_1: Handle<Mesh>,
    pub knight_2: Handle<Mesh>,
    pub rook: Handle<Mesh>,
    pub bishop: Handle<Mesh>,
    pub queen: Handle<Mesh>,
}

fn load_piece_meshes(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(PieceMeshes {
        king: asset_server.load("models/chess_kit/pieces.glb#Mesh0/Primitive0"),
        king_cross: asset_server.load("models/chess_kit/pieces.glb#Mesh1/Primitive0"),
        pawn: asset_server.load("models/chess_kit/pieces.glb#Mesh2/Primitive0"),
        knight_1: asset_server.load("models/chess_kit/pieces.glb#Mesh3/Primitive0"),
        knight_2: asset_server.load("models/chess_kit/pieces.glb#Mesh4/Primitive0"),
        rook: asset_server.load("models/chess_kit/pieces.glb#Mesh5/Primitive0"),
        bishop: asset_server.load("models/chess_kit/pieces.glb#Mesh6/Primitive0"),
        queen: asset_server.load("models/chess_kit/pieces.glb#Mesh7/Primitive0"),
    });
}

// No offsets needed - the GLB models should be centered correctly at their local origin.
// Pieces are spawned directly at their board positions without offsets.
// This matches the reference implementation at references/bevy-3d-chess/src/main.rs

/// Convert chess board position to world position.
///
/// Uses the same coordinate system as the reference implementation:
/// - World X = 7 - rank (so rank 0 is at X=7, rank 7 is at X=0)
/// - World Z = file (so file 0 is at Z=0, file 7 is at Z=7)
/// - Y = board surface height
///
/// This places a1 (file=0, rank=0) at world position (7, 0.05, 0)
/// and h8 (file=7, rank=7) at world position (0, 0.05, 7).
pub fn board_pos_to_world(file: u8, rank: u8) -> Vec3 {
    Vec3::new((7 - rank) as f32, PIECE_ON_BOARD_Y, file as f32)
}

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
    _visual_offset: Vec3, // Kept for API compatibility, no longer used
) {
    let (file, rank) = position;
    // World position using reference coordinate system
    let world_pos = board_pos_to_world(file, rank);

    // DEBUG: Log spawn position for verification
    info!(
        "[SPAWN] {:?} {:?} at file={}, rank={} -> world_pos={:?}",
        color, piece_type, file, rank, world_pos
    );

    // Spawn at calculated board position
    match piece_type {
        PieceType::King => spawn_king(
            commands,
            material,
            color,
            world_pos,
            meshes,
            Vec3::ZERO,
            file,
            rank,
        ),
        PieceType::Queen => spawn_queen(
            commands,
            material,
            color,
            world_pos,
            meshes,
            Vec3::ZERO,
            file,
            rank,
        ),
        PieceType::Rook => spawn_rook(
            commands,
            material,
            color,
            world_pos,
            meshes,
            Vec3::ZERO,
            file,
            rank,
        ),
        PieceType::Bishop => spawn_bishop(
            commands,
            material,
            color,
            world_pos,
            meshes,
            Vec3::ZERO,
            file,
            rank,
        ),
        PieceType::Knight => spawn_knight(
            commands,
            material,
            color,
            world_pos,
            meshes,
            Vec3::ZERO,
            file,
            rank,
        ),
        PieceType::Pawn => spawn_pawn(
            commands,
            material,
            color,
            world_pos,
            meshes,
            Vec3::ZERO,
            file,
            rank,
        ),
    }
}

/// Creates a transform for piece mesh.
///
/// Applies the mesh offset (from GLB baked positions) and sets the scale.
/// This matches the showcase implementation exactly.
fn piece_mesh_transform(_visual_offset: Vec3, mesh_offset: Vec3) -> Transform {
    // Apply mesh offset directly - this centers the mesh on the piece parent
    // The showcase does: offset * OFFSET_RATIO, but game uses full scale
    // so we apply the offset directly without additional scaling
    let mut t = Transform::from_translation(mesh_offset);
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
        // White: Base 0° + (-90°) = -90° → rotates from +X to +Z (toward black's side)
        PieceColor::White => Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2),
        // Black: Base 180° + 90° = 270° (-90°) → rotates from -X to -Z (toward white's side)
        PieceColor::Black => Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
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
/// Spawns a piece mesh with proper offset correction.
///
/// # Arguments
/// * `$parent` - The parent commands scope
/// * `$mesh` - The mesh handle to spawn
/// * `$material` - The material handle
/// * `$visual_offset` - Additional visual offset (usually ZERO)
/// * `$mesh_offset` - GLB baked-position counter-offset (mesh-specific constant)
macro_rules! spawn_piece_visual {
    ($parent:expr, $mesh:expr, $material:expr, $visual_offset:expr, $mesh_offset:expr) => {
        $parent.spawn((
            Mesh3d($mesh),
            MeshMaterial3d($material),
            piece_mesh_transform($visual_offset, $mesh_offset),
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
    visual_offset: Vec3,
    file: u8,
    rank: u8,
) {
    use crate::core::{DespawnOnExit, GameState};

    let mesh = piece_meshes.king.clone();
    let mesh_cross = piece_meshes.king_cross.clone(); // The cross on top of the king

    commands
        .spawn((
            Piece::new(piece_color, PieceType::King, file, rank),
            Transform::from_translation(world_pos).with_rotation(piece_rotation(piece_color)),
            GlobalTransform::default(),
            Visibility::default(),
            DespawnOnExit(GameState::InGame),
            PointerInteraction::default(),
            bevy::picking::Pickable::default(), // Required for picking
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
            spawn_piece_visual!(
                parent,
                mesh,
                material.clone(),
                visual_offset,
                KING_BASE_MESH_OFFSET
            );
            spawn_piece_visual!(
                parent,
                mesh_cross,
                material,
                visual_offset,
                KING_CROSS_MESH_OFFSET
            );
        });
}

pub fn spawn_knight(
    commands: &mut Commands,
    material: Handle<StandardMaterial>,
    piece_color: PieceColor,
    world_pos: Vec3,
    piece_meshes: &PieceMeshes,
    visual_offset: Vec3,
    file: u8,
    rank: u8,
) {
    use crate::core::GameState;

    let mesh_1 = piece_meshes.knight_1.clone();
    let mesh_2 = piece_meshes.knight_2.clone();

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
            spawn_piece_visual!(
                parent,
                mesh_1,
                material.clone(),
                visual_offset,
                KNIGHT_1_MESH_OFFSET
            );
            spawn_piece_visual!(
                parent,
                mesh_2,
                material,
                visual_offset,
                KNIGHT_2_MESH_OFFSET
            );
        });
}

pub fn spawn_queen(
    commands: &mut Commands,
    material: Handle<StandardMaterial>,
    piece_color: PieceColor,
    world_pos: Vec3,
    piece_meshes: &PieceMeshes,
    visual_offset: Vec3,
    file: u8,
    rank: u8,
) {
    use crate::core::GameState;

    let mesh = piece_meshes.queen.clone();

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
            spawn_piece_visual!(parent, mesh, material, visual_offset, QUEEN_MESH_OFFSET);
        });
}

pub fn spawn_bishop(
    commands: &mut Commands,
    material: Handle<StandardMaterial>,
    piece_color: PieceColor,
    world_pos: Vec3,
    piece_meshes: &PieceMeshes,
    visual_offset: Vec3,
    file: u8,
    rank: u8,
) {
    use crate::core::GameState;

    let mesh = piece_meshes.bishop.clone();

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
            spawn_piece_visual!(parent, mesh, material, visual_offset, BISHOP_MESH_OFFSET);
        });
}

pub fn spawn_rook(
    commands: &mut Commands,
    material: Handle<StandardMaterial>,
    piece_color: PieceColor,
    world_pos: Vec3,
    piece_meshes: &PieceMeshes,
    visual_offset: Vec3,
    file: u8,
    rank: u8,
) {
    use crate::core::GameState;

    let mesh = piece_meshes.rook.clone();

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
            spawn_piece_visual!(parent, mesh, material, visual_offset, ROOK_MESH_OFFSET);
        });
}

pub fn spawn_pawn(
    commands: &mut Commands,
    material: Handle<StandardMaterial>,
    piece_color: PieceColor,
    world_pos: Vec3,
    piece_meshes: &PieceMeshes,
    visual_offset: Vec3,
    file: u8,
    rank: u8,
) {
    use crate::core::GameState;

    let mesh = piece_meshes.pawn.clone();

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
            spawn_piece_visual!(parent, mesh, material, visual_offset, PAWN_MESH_OFFSET);
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
